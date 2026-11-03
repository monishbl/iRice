import QtQuick
import Quickshell
import qs.common
import qs.services

// IslandHistoryCard in the Material look, laid out like
// MaterialNotificationCard: the alert's icon on a tonal circle (a device's
// picture in its place), what kind of alert and when, the title and detail,
// and the alert's own button (Open a download or a drive) as a tonal pill.
// The background is up to the parent.
Item {
    id: root

    required property var alert
    property bool hovered: hover.hovered
    signal closeRequested
    // A stack of alerts: as MaterialNotificationCard's
    property int groupCount: 1
    property bool groupOpen: false
    property bool tapExpands: false
    signal groupToggled
    readonly property var act: Island.historyActs[alert?.hid ?? 0] ?? null

    // What kind of alert, as the small caption
    readonly property var kinds: ({ device: "Bluetooth", usb: "USB", display: "Display", audio: "Sound output", charging: "Power", battery: "Battery", temperature: "Temperature", resources: "System", network: "Network", transfer: "Downloads", timer: "Timer", recording: "Screen recording", focus: "Focus", sysupdate: "Software update", custom: "iRice" })

    implicitHeight: column.implicitHeight + 32

    HoverHandler { id: hover }
    // A collapsed stack opens with a click anywhere on it
    TapHandler {
        enabled: root.tapExpands
        onTapped: root.groupToggled()
    }

    Item {
        id: badge
        x: 16
        y: 16
        width: 40
        height: 40
        // (tinted with the alert's own colour, as on the island)
        Rectangle {
            anchors.fill: parent
            visible: !root.alert?.picture
            radius: width / 2
            color: Qt.tint(Theme.m3.surfaceContainerHighest, Qt.alpha(root.alert?.color ?? Theme.m3.primary, 0.22))
        }
        Image {
            anchors.fill: parent
            visible: !!root.alert?.picture
            source: root.alert?.picture ?? ""
            sourceSize: Qt.size(80, 80)
            fillMode: Image.PreserveAspectFit
            smooth: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !root.alert?.picture
            name: root.alert?.icon ?? "bell"
            fill: true
            size: 20
            color: root.alert?.color ?? Theme.m3.primary
        }
    }

    Row {
        id: corner
        anchors { right: parent.right; rightMargin: 12; top: parent.top; topMargin: 12 }
        spacing: 4
        IconButton {
            width: 28
            height: 28
            visible: root.hovered || activeFocus
            icon: "x"
            iconSize: 16
            iconColor: Theme.m3.onSurfaceVariant
            accessibleName: "Remove " + (root.alert?.title ?? "alert")
            onClicked: root.closeRequested()
        }
        MaterialExpander {
            visible: root.groupCount > 1
            count: root.groupCount
            open: root.groupOpen
            onClicked: root.groupToggled()
        }
    }

    Column {
        id: column
        anchors { left: badge.right; leftMargin: 12; right: parent.right; rightMargin: 16; top: parent.top; topMargin: 16 }
        spacing: 2

        Text {
            width: parent.width - (corner.width > 0 ? corner.width + 4 : 0)
            height: 20
            verticalAlignment: Text.AlignVCenter
            text: {
                const a = Island.ago(root.alert?.at ?? 0, clock.date.getTime());
                return (root.kinds[root.alert?.kind ?? ""] ?? "iRice") + " · " + (a === "now" ? "now" : /^\d+[mh]$/.test(a) ? a + " ago" : a);
            }
            elide: Text.ElideRight
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
            SystemClock { id: clock; precision: SystemClock.Minutes }
        }
        Text {
            width: parent.width
            text: root.alert?.title ?? ""
            elide: Text.ElideRight
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: 14; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: [root.alert?.subtitle ?? "", root.alert?.battery !== undefined ? "Battery " + Math.round(root.alert.battery * 100) + "%" : "", (root.alert?.times ?? 1) > 1 ? root.alert.times + " times" : ""].filter(x => !!x).join(" · ")
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: Theme.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 14; features: { "tnum": 1 } }
        }
        Item {
            visible: root.act !== null
            width: parent.width
            height: 42
            Rectangle {
                id: actButton
                anchors.bottom: parent.bottom
                width: actLabel.implicitWidth + 32
                height: 32
                transform: pressFx1.effects
                ElasticControl { id: pressFx1; pressed: actMouse.pressed }
                radius: actMouse.pressed ? 8 : 16
                Behavior on radius { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }
                color: actMouse.pressed ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.1)) : actMouse.containsMouse ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, 0.08)) : Theme.m3.secondaryContainer
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                activeFocusOnTab: root.act !== null
                Keys.onReturnPressed: root.act?.act()
                Keys.onSpacePressed: root.act?.act()
                Accessible.role: Accessible.Button
                Accessible.name: root.act?.text ?? ""
                FocusRing { radius: actButton.radius + 5 }
                Text {
                    id: actLabel
                    anchors.centerIn: parent
                    text: root.act?.text ?? ""
                    color: Theme.m3.onSecondaryContainer
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
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
}
