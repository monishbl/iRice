import QtQuick
import Quickshell.Services.UPower
import qs.common
import qs.services

// The quiet last line of the expanded island: battery, CPU (with the live
// package temperature, reddish when running hot) and memory, and the next
// calendar event. Staggers in after the activities.
Item {
    id: root

    property int delay: 0
    property bool shown: false
    readonly property var next: shown ? Calendar.upcoming(1)[0] ?? null : null
    implicitHeight: 22

    // "8:00 AM", "Tomorrow 8:00 AM", "Fri 8:00 AM"
    function when(e) {
        const now = new Date();
        const day = d => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
        const diff = Math.round((day(e.start) - day(now)) / 86400000);
        const prefix = diff === 0 ? "" : diff === 1 ? "Tomorrow " : Qt.formatDate(e.start, "ddd") + " ";
        return e.allDay ? (diff === 0 ? "Today" : prefix.trim()) : prefix + Qt.formatTime(e.start, "h:mm AP");
    }

    // Make sure CPU and memory are sampled while this shows them
    property bool counted: false
    onShownChanged: {
        if (shown !== counted) {
            Resources.desktopUsers += shown ? 1 : -1;
            counted = shown;
        }
    }
    Component.onDestruction: if (counted) Resources.desktopUsers--

    property real appear: shown ? 1 : 0
    Behavior on appear {
        SequentialAnimation {
            PauseAnimation { duration: root.shown ? root.delay : 0 }
            NumberAnimation { duration: 280; easing.type: Easing.OutCubic }
        }
    }
    opacity: appear
    transform: Translate { y: (1 - root.appear) * 8 }

    Row {
        id: stats
        anchors.verticalCenter: parent.verticalCenter
        spacing: 14
        Repeater {
            model: [
                ...(Island.hasBattery ? [{ icon: Island.charging ? "battery-charging" : "battery-high", text: Math.round(Island.level * 100) + "%" }] : []),
                { icon: "cpu", text: Math.round(Resources.cpu * 100) + "%" },
                ...(Resources.temperature > 0 ? [{ icon: "thermometer-simple", text: Math.round(Resources.temperature) + "°", hot: Island.hot }] : []),
                { icon: "memory", text: Math.round(Resources.memory * 100) + "%" }
            ]
            Row {
                required property var modelData
                spacing: 4
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: parent.modelData.icon
                    size: 13
                    color: parent.modelData.hot ? Qt.rgba(1, 0.55, 0.5, 0.85) : Qt.rgba(1, 1, 1, 0.6)
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData.text
                    color: parent.modelData.hot ? Qt.rgba(1, 0.55, 0.5, 0.85) : Qt.rgba(1, 1, 1, 0.6)
                    font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                }
            }
        }
    }

    Text {
        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
        // What's left after the stats (hidden when too little to read)
        width: Math.min(implicitWidth, root.width - stats.width - 16)
        visible: root.next !== null && width >= 60
        text: root.next ? root.next.title + " · " + root.when(root.next) : ""
        elide: Text.ElideRight
        color: Qt.rgba(1, 1, 1, 0.6)
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
