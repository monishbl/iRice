import QtQuick
import Quickshell
import qs.common
import qs.services

// One island alert (`alert`) in Notification Center's iRice group (Island.history),
// laid out like a NotificationCard: its icon, what kind of alert and when,
// the title and detail. A close button on hover; the alert's own button
// (Open a download or a drive) while the shell remembers it.
Item {
    id: root

    required property var alert
    property bool hovered: hover.hovered
    signal closeRequested
    readonly property var act: Island.historyActs[alert?.hid ?? 0] ?? null

    // What kind of alert, as the small caption
    readonly property var kinds: ({ device: "Bluetooth", usb: "USB", display: "Display", audio: "Sound output", charging: "Power", battery: "Battery", temperature: "Temperature", resources: "System", network: "Network", transfer: "Downloads", timer: "Timer", recording: "Screen recording", focus: "Focus", sysupdate: "Software update", custom: "iRice" })

    implicitHeight: column.implicitHeight + 24

    HoverHandler { id: hover }

    Rectangle {
        id: badge
        x: 12
        y: 12
        width: 36
        height: 36
        radius: 18
        color: Qt.alpha(root.alert?.color ?? Theme.accent, 0.18)
        Icon {
            anchors.centerIn: parent
            name: root.alert?.icon ?? "bell"
            fill: true
            size: 20
            color: root.alert?.color ?? Theme.accent
        }
    }

    Column {
        id: column
        anchors { left: badge.right; leftMargin: 10; right: parent.right; rightMargin: 12; top: parent.top; topMargin: 12 }
        spacing: 2

        Item {
            width: parent.width
            height: kind.implicitHeight
            Text {
                id: kind
                anchors { left: parent.left; right: time.left; rightMargin: 8 }
                text: root.kinds[root.alert?.kind ?? ""] ?? "iRice"
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold; capitalization: Font.AllUppercase; letterSpacing: 0.3 }
            }
            Text {
                id: time
                anchors.right: parent.right
                text: {
                    const a = Island.ago(root.alert?.at ?? 0, clock.date.getTime());
                    return a === "now" ? "now" : /^\d+[mh]$/.test(a) ? a + " ago" : a;
                }
                color: Theme.color.tertiary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                SystemClock { id: clock; precision: SystemClock.Minutes }
            }
        }
        Text {
            width: parent.width
            text: root.alert?.title ?? ""
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: [root.alert?.subtitle ?? "", root.alert?.battery !== undefined ? "Battery " + Math.round(root.alert.battery * 100) + "%" : "", (root.alert?.times ?? 1) > 1 ? root.alert.times + " times" : ""].filter(x => !!x).join(" · ")
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: Theme.color.label
            opacity: 0.85
            font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
        }
        Row {
            visible: root.act !== null
            topPadding: 6
            Rectangle {
                width: actLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: actMouse.pressed ? Theme.color.fillPressed : actMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Text {
                    id: actLabel
                    anchors.centerIn: parent
                    text: root.act?.text ?? ""
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
                MouseArea {
                    id: actMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.act?.act()
                }
            }
        }
    }

    // Close button, top left like a notification's
    Rectangle {
        x: -6
        y: -6
        width: 22
        height: 22
        radius: 11
        visible: root.hovered
        color: Theme.dark ? Qt.rgba(0.22, 0.22, 0.25, 1) : "white"
        border.width: 1
        border.color: Theme.color.separator
        Icon {
            anchors.centerIn: parent
            name: "x"
            bold: true
            size: 11
            color: Theme.color.secondary
        }
        MouseArea {
            anchors { fill: parent; margins: -3 }
            onClicked: root.closeRequested()
        }
        Accessible.role: Accessible.Button
        Accessible.name: "Remove " + (root.alert?.title ?? "alert")
    }
}
