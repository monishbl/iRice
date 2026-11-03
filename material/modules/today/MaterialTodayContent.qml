import QtQuick
import qs.common
import qs.modules.notifications
import qs.services

// TodayContent in the Material look: the day as Material You widgets. Big
// rounded cards: the greeting on primaryContainer with the date cut into a
// cookie, the weather now on tertiaryContainer around a sunny shape, then
// today's events, the open to-dos and the hours ahead on surface containers.
// Pin for today (tonal) and Close (filled) at the bottom. Same data, keys
// and stagger as TodayContent: `rows` (0 → 1) brings the cards in one after
// another; Esc and Enter close; Tab moves between the buttons.
FocusScope {
    id: root

    property real rows: 1
    readonly property int pad: 24
    readonly property int gap: 16

    // Today's events (past ones dimmed) and open to-dos, soonest first (a
    // plain property, as in TodayContent: opened at login, a clock object
    // made later left the lists empty for a minute)
    property date now: new Date()
    readonly property var events: Calendar.eventsOn(now)
    readonly property var todos: Todos.sorted(Todos.list.filter(t => !t.done)).slice(0, 5)
    Timer {
        interval: 60 * 1000
        running: true
        repeat: true
        onTriggered: root.now = new Date()
    }

    Keys.onEscapePressed: Today.close()
    Keys.onReturnPressed: Today.close()
    Keys.onEnterPressed: Today.close()

    // The grid: two columns of cards beside a weather column
    readonly property real sideWidth: Math.min(300, Math.round(width * 0.36))
    readonly property real mainWidth: width - 2 * pad - sideWidth - gap
    readonly property real heroHeight: Math.max(136, Math.min(176, Math.round(height * 0.28)))
    readonly property real lowerY: pad + heroHeight + gap
    readonly property real lowerHeight: buttons.y - gap - lowerY

    // A card rising into place: order n starts 40 ms after n - 1, fading in
    // as it rises 8 px (the stagger is passed in: inline components don't
    // see the file's ids)
    component Rise: Item {
        property int order: 0
        property real stagger: 1
        readonly property real k: Math.max(0, Math.min(1, (stagger * 620 - order * 40) / 300))
        opacity: k
        transform: Translate { y: Math.pow(1 - k, 3) * 8 }
    }

    // A Material button: filled (primary) or tonal, 40 high and round, the
    // corners tightening while pressed
    component MButton: Rectangle {
        id: button
        property string text
        property string icon
        property bool primary: false
        signal activated
        implicitWidth: buttonRow.implicitWidth + 48
        implicitHeight: 40
        transform: pressFx1.effects
        ElasticControl { id: pressFx1; pressed: buttonMouse.pressed }
        radius: buttonMouse.pressed ? 12 : 20
        Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
        readonly property color base: primary ? Theme.m3.primary : Theme.m3.secondaryContainer
        readonly property color ink: primary ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer
        color: buttonMouse.pressed ? Qt.tint(base, Qt.alpha(ink, 0.1)) : buttonMouse.containsMouse ? Qt.tint(base, Qt.alpha(ink, 0.08)) : base
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: true
        Keys.onReturnPressed: button.activated()
        Keys.onEnterPressed: button.activated()
        Keys.onSpacePressed: button.activated()
        Accessible.role: Accessible.Button
        Accessible.name: text
        FocusRing { radius: button.radius + 5 }
        Row {
            id: buttonRow
            anchors.centerIn: parent
            spacing: 8
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                visible: button.icon !== ""
                name: button.icon
                size: 18
                color: button.ink
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: button.text
                color: button.ink
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
        }
        MouseArea {
            id: buttonMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.activated()
        }
    }

    // A card's heading
    component Heading: Text {
        color: Theme.m3.onSurface
        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
    }

    // ── The greeting: primaryContainer, the date in a cookie ───────────
    Rise {
        order: 0
        stagger: root.rows
        x: root.pad
        y: root.pad
        width: root.mainWidth
        height: root.heroHeight
        Rectangle {
            anchors.fill: parent
            radius: 28
            color: Theme.m3.primaryContainer
        }
        Item {
            id: dateShape
            x: 20
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(120, parent.height - 40)
            height: width
            MaterialShape {
                anchors.fill: parent
                shape: "cookie12"
                color: Theme.m3.primary
                // (turns into place as it arrives, then holds still)
                spin: (1 - parent.parent.k) * -40
            }
            Column {
                anchors.centerIn: parent
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Qt.formatDate(root.now, "MMM").toUpperCase()
                    color: Theme.m3.onPrimary
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold; letterSpacing: 1 }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.now.getDate()
                    color: Theme.m3.onPrimary
                    font { family: Theme.font.family; pixelSize: 44; weight: Font.Medium; features: { "tnum": 1 } }
                }
            }
        }
        Column {
            anchors { left: dateShape.right; leftMargin: 24; right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
            spacing: 4
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: Today.greeting(root.now)
                color: Theme.m3.onPrimaryContainer
                font { family: Theme.font.family; pixelSize: 28; weight: Font.Medium; letterSpacing: -0.2 }
            }
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: Qt.formatDate(root.now, "dddd, d MMMM")
                color: Theme.m3.onPrimaryContainer
                font { family: Theme.font.family; pixelSize: 16 }
            }
            // What the day holds, in a few words
            Text {
                width: parent.width
                topPadding: 8
                elide: Text.ElideRight
                text: {
                    const e = root.events.length, t = root.todos.length;
                    if (e === 0 && t === 0)
                        return "A clear day";
                    const parts = [];
                    if (e > 0)
                        parts.push(e + (e === 1 ? " event" : " events"));
                    if (t > 0)
                        parts.push(t + (t === 1 ? " to-do" : " to-dos"));
                    return parts.join(" · ");
                }
                color: Theme.m3.onPrimaryContainer
                opacity: 0.8
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
            }
        }
    }

    // ── The weather now: tertiaryContainer around a sunny shape ────────
    Rise {
        order: 1
        stagger: root.rows
        x: root.pad + root.mainWidth + root.gap
        y: root.pad
        width: root.sideWidth
        height: root.heroHeight
        Rectangle {
            anchors.fill: parent
            radius: 28
            color: Theme.m3.tertiaryContainer
        }
        Item {
            id: sun
            x: 20
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(96, parent.height - 48)
            height: width
            MaterialShape {
                anchors.fill: parent
                shape: Weather.ready ? "verySunny" : "cookie4"
                color: Theme.m3.tertiary
                spin: (1 - parent.parent.k) * 45
            }
            Icon {
                anchors.centerIn: parent
                name: Weather.ready ? Weather.icon : "cloud"
                fill: true
                size: Math.round(sun.width * 0.42)
                color: Theme.m3.onTertiary
            }
        }
        Column {
            anchors { left: sun.right; leftMargin: 16; right: parent.right; rightMargin: 20; verticalCenter: parent.verticalCenter }
            spacing: 2
            Text {
                visible: Weather.ready
                text: Weather.temp + Weather.unit
                color: Theme.m3.onTertiaryContainer
                font { family: Theme.font.family; pixelSize: 40; weight: Font.Medium; features: { "tnum": 1 } }
            }
            Text {
                width: parent.width
                visible: Weather.ready
                elide: Text.ElideRight
                text: Weather.condition
                color: Theme.m3.onTertiaryContainer
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
            Text {
                width: parent.width
                visible: Weather.ready
                elide: Text.ElideRight
                text: "H " + Weather.high + "° · L " + Weather.low + "°" + (Weather.rainChance > 0 ? " · " + Weather.rainChance + "% rain" : "")
                color: Theme.m3.onTertiaryContainer
                opacity: 0.8
                font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
            }
            Text {
                width: parent.width
                visible: !Weather.ready
                wrapMode: Text.Wrap
                text: Weather.error === "Offline" ? "Offline. The weather shows once you're connected." : "The weather isn't available right now."
                color: Theme.m3.onTertiaryContainer
                font { family: Theme.font.family; pixelSize: 14 }
            }
        }
    }

    // ── Today's events ─────────────────────────────────────────────────
    Rise {
        id: eventsCard
        order: 2
        stagger: root.rows
        x: root.pad
        y: root.lowerY
        width: (root.mainWidth - root.gap) / 2
        height: root.lowerHeight
        // As many as fit (48 high, 4 apart, under the heading)
        readonly property int fits: Math.max(1, Math.floor((height - 16 - 40 - 12) / 52))
        Rectangle {
            anchors.fill: parent
            radius: 24
            color: Theme.m3.surfaceContainerHigh
        }
        Heading {
            x: 20
            y: 16
            height: 24
            verticalAlignment: Text.AlignVCenter
            text: "Today"
        }
        Text {
            visible: root.events.length === 0
            anchors { left: parent.left; right: parent.right; margins: 20; top: parent.top; topMargin: 52 }
            wrapMode: Text.Wrap
            text: "Nothing on your calendar today"
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14 }
        }
        Column {
            x: 12
            y: 52
            width: parent.width - 24
            spacing: 4
            Repeater {
                model: root.events.slice(0, eventsCard.fits)
                Rectangle {
                    id: ev
                    required property var modelData
                    readonly property bool past: !modelData.allDay && modelData.end < root.now
                    width: parent.width
                    height: 48
                    radius: 12
                    color: Theme.m3.surfaceContainerHighest
                    opacity: past ? 0.5 : 1
                    Rectangle {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: 4
                        height: parent.height - 20
                        radius: 2
                        color: ev.modelData.color || Theme.m3.primary
                    }
                    Column {
                        anchors { left: parent.left; leftMargin: 24; right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: ev.modelData.title
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: Calendar.timeText(ev.modelData)
                            color: Theme.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                        }
                    }
                }
            }
        }
        Text {
            visible: root.events.length > eventsCard.fits
            anchors { left: parent.left; leftMargin: 20; bottom: parent.bottom; bottomMargin: 14 }
            text: "+" + (root.events.length - eventsCard.fits) + " more"
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
        }
    }

    // ── Open to-dos ────────────────────────────────────────────────────
    Rise {
        id: todosCard
        order: 3
        stagger: root.rows
        x: root.pad + eventsCard.width + root.gap
        y: root.lowerY
        width: eventsCard.width
        height: root.lowerHeight
        readonly property int fits: Math.max(1, Math.floor((height - 16 - 40 - 12) / 48))
        Rectangle {
            anchors.fill: parent
            radius: 24
            color: Theme.m3.surfaceContainerHigh
        }
        Heading {
            x: 20
            y: 16
            height: 24
            verticalAlignment: Text.AlignVCenter
            text: "To-dos"
        }
        Text {
            visible: root.todos.length === 0
            anchors { left: parent.left; right: parent.right; margins: 20; top: parent.top; topMargin: 52 }
            wrapMode: Text.Wrap
            text: Todos.list.length === 0 ? "No to-dos. Add them in Notification Center." : "All done"
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14 }
        }
        Column {
            x: 20
            y: 52
            width: parent.width - 40
            Repeater {
                model: root.todos.slice(0, todosCard.fits)
                Item {
                    id: todo
                    required property var modelData
                    width: parent.width
                    height: 48
                    MaterialCheck {
                        id: tick
                        anchors.verticalCenter: parent.verticalCenter
                        accessibleName: todo.modelData.text
                        onToggled: Todos.update(todo.modelData, { done: !todo.modelData.done })
                    }
                    Text {
                        anchors { left: tick.right; leftMargin: 16; right: due.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        elide: Text.ElideRight
                        text: todo.modelData.text
                        color: Theme.m3.onSurface
                        font { family: Theme.font.family; pixelSize: 14 }
                    }
                    Text {
                        id: due
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        visible: !!todo.modelData.due
                        readonly property date at: new Date(todo.modelData.due ?? 0)
                        text: todo.modelData.allDay ? (Today.dayKey(at) === Today.day ? "Today" : Qt.formatDate(at, "ddd d MMM")) : Qt.formatTime(at, "h:mm AP")
                        color: (todo.modelData.due ?? Infinity) < root.now.getTime() ? Theme.m3.error : Theme.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }

    // ── The hours ahead ────────────────────────────────────────────────
    Rise {
        order: 4
        stagger: root.rows
        x: root.pad + root.mainWidth + root.gap
        y: root.lowerY
        width: root.sideWidth
        height: root.lowerHeight
        Rectangle {
            anchors.fill: parent
            radius: 24
            color: Theme.m3.surfaceContainerHigh
        }
        Heading {
            x: 20
            y: 16
            height: 24
            verticalAlignment: Text.AlignVCenter
            text: "Later today"
        }
        Text {
            visible: !Weather.ready
            anchors { left: parent.left; right: parent.right; margins: 20; top: parent.top; topMargin: 52 }
            wrapMode: Text.Wrap
            text: "The forecast shows here once the weather is available."
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14 }
        }
        Column {
            x: 20
            y: 52
            width: parent.width - 40
            Repeater {
                model: Weather.ready ? Weather.hourly : []
                Item {
                    id: hour
                    required property var modelData
                    width: parent.width
                    height: 44
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 72
                        text: hour.modelData.time
                        color: Theme.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: 14; features: { "tnum": 1 } }
                    }
                    Icon {
                        x: 80
                        anchors.verticalCenter: parent.verticalCenter
                        name: hour.modelData.icon
                        fill: true
                        size: 20
                        color: Theme.m3.onSurface
                    }
                    Text {
                        x: 112
                        anchors.verticalCenter: parent.verticalCenter
                        visible: (hour.modelData.rain ?? 0) >= 20
                        text: hour.modelData.rain + "%"
                        color: Theme.m3.primary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                    Text {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        text: hour.modelData.temp + Weather.unit
                        color: Theme.m3.onSurface
                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                }
            }
        }
    }

    // ── Buttons ────────────────────────────────────────────────────────
    Rise {
        id: buttons
        order: 6
        stagger: root.rows
        anchors { right: parent.right; rightMargin: root.pad; bottom: parent.bottom; bottomMargin: root.pad }
        width: buttonsRow.implicitWidth
        height: buttonsRow.implicitHeight
        Row {
            id: buttonsRow
            spacing: 8
            MButton {
                // the widget is on the desktop every day anyway
                visible: !Today.widgetAlways
                text: "Pin for today"
                icon: "push-pin"
                onActivated: Today.pin()
            }
            MButton {
                text: "Close"
                primary: true
                focus: true
                onActivated: Today.close()
            }
        }
    }
}
