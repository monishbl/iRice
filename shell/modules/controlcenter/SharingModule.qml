import QtQuick
import qs.common
import qs.services

// The Sharing module's bigger sizes (1×1 is the usual round toggle):
//   2×1  the three stops: Off · 10 min · On (both LocalSend and Quick Share)
//   2×2  the stops, then up to three devices nearby: click one, pick files,
//        they're sent
//   4×1  the stops beside the devices nearby, as chips
// Devices are only looked for while the module is on screen.
Item {
    id: root

    required property int cols
    required property int rows
    property bool preview: false // the gallery: no looking, no actions
    property bool editing: false
    signal openPage

    readonly property bool showsDevices: rows === 2 || cols >= 4
    readonly property bool active: visible && showsDevices && !preview && Panels.open === "controlCenter"
    property bool lookingHere: false
    onActiveChanged: look(active)
    Component.onCompleted: look(active)
    Component.onDestruction: look(false)
    function look(on) {
        if (on !== lookingHere) {
            lookingHere = on;
            Sharing.look(on);
        }
    }
    readonly property var nearby: preview ? [] : Sharing.devices.slice(0, 3)
    readonly property bool interactive: !preview && !editing

    // ── 2×1: just the stops ─────────────────────────────────────────────
    LevelControl {
        visible: root.rows === 1 && root.cols < 4
        anchors { fill: parent; margins: 8 }
        value: Sharing.level
        enabled_: root.interactive
        onPicked: v => Sharing.setLevel(v)
    }

    // ── 2×2: title, stops, devices ──────────────────────────────────────
    Column {
        visible: root.rows === 2
        anchors { fill: parent; margins: 12 }
        spacing: 10
        Row {
            width: parent.width
            spacing: 8
            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "share-network"
                fill: Sharing.on
                size: 16
                color: Sharing.on ? Theme.accentInk : Theme.color.label
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 24
                text: "Sharing · " + Sharing.levelText
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
            }
        }
        LevelControl {
            width: parent.width
            height: 32
            value: Sharing.level
            enabled_: root.interactive
            onPicked: v => Sharing.setLevel(v)
        }
        Repeater {
            model: root.nearby
            DeviceRow {
                required property var modelData
                device: modelData
                width: parent.width
            }
        }
        Text {
            visible: root.nearby.length === 0
            width: parent.width
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            text: root.preview ? "Devices nearby show here" : "No devices nearby"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    // ── 4×1: stops beside device chips ──────────────────────────────────
    Row {
        visible: root.rows === 1 && root.cols >= 4
        anchors { fill: parent; margins: 8 }
        spacing: 10
        LevelControl {
            width: Math.min(200, parent.width * 0.48)
            height: parent.height
            value: Sharing.level
            enabled_: root.interactive
            onPicked: v => Sharing.setLevel(v)
        }
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Repeater {
                model: root.nearby
                Rectangle {
                    id: chip
                    required property var modelData
                    readonly property var sending: Sharing.progressFor(modelData)
                    width: Math.min(110, chipText.implicitWidth + 36)
                    height: 30
                    radius: 15
                    color: chipMouse.pressed ? Theme.color.fillPressed : chipMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                    Icon {
                        id: chipIcon
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        name: chip.modelData.type === "mobile" ? "device-mobile" : chip.modelData.type === "laptop" ? "laptop" : "desktop"
                        size: 13
                        color: Theme.color.secondary
                    }
                    Text {
                        id: chipText
                        anchors { left: chipIcon.right; leftMargin: 6; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                        text: chip.sending ? Math.round(100 * (chip.sending.total > 0 ? chip.sending.done / chip.sending.total : 0)) + "%" : chip.modelData.name
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium; features: { "tnum": 1 } }
                    }
                    MouseArea {
                        id: chipMouse
                        anchors.fill: parent
                        enabled: root.interactive && !chip.sending
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Sharing.pickFor(chip.modelData)
                    }
                }
            }
            Text {
                visible: root.nearby.length === 0
                anchors.verticalCenter: parent.verticalCenter
                text: root.preview ? "Devices nearby" : "No devices nearby"
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
    }

    // A device nearby: icon, name, how it's reached; click to send files
    component DeviceRow: Rectangle {
        id: row
        property var device
        readonly property var sending: Sharing.progressFor(device)
        height: 30
        radius: 8
        color: rowMouse.pressed ? Theme.color.fillPressed : rowMouse.containsMouse ? Theme.color.fill : "transparent"
        Icon {
            id: rowIcon
            x: 6
            anchors.verticalCenter: parent.verticalCenter
            name: row.device.type === "mobile" ? "device-mobile" : row.device.type === "laptop" ? "laptop" : "desktop"
            size: 15
            color: Theme.color.secondary
        }
        Text {
            anchors { left: rowIcon.right; leftMargin: 8; right: rowStatus.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
            text: row.device.name
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Text {
            id: rowStatus
            anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
            text: row.sending ? (row.sending.waiting ? "Waiting…" : Math.round(100 * (row.sending.total > 0 ? row.sending.done / row.sending.total : 0)) + "%") : "Send…"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
        }
        MouseArea {
            id: rowMouse
            anchors.fill: parent
            enabled: root.interactive && !row.sending
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Sharing.pickFor(row.device)
        }
    }
}
