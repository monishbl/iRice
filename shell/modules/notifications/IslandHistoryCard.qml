import QtQuick
import Quickshell
import qs.common
import qs.services
import "NotificationGroups.js" as NotificationGroups

// One island alert (`alert`) in Notification Center's Tessera group (Island.history),
// laid out like a NotificationCard: its icon, what kind of alert and when,
// the title and detail. A close button on hover; the alert's own button
// (Open a download or a drive) while the shell remembers it.
Item {
    id: root

    required property var alert
    property bool hovered: hover.hovered
    signal closeRequested
    readonly property var act: Island.historyActs[alert?.hid ?? 0] ?? null


    implicitHeight: column.implicitHeight + 24

    HoverHandler { id: hover }

    Rectangle {
        id: badge
        x: 12
        y: 12
        width: 36
        height: 36
        radius: 18
        color: root.alert?.picture ? "transparent" : Qt.alpha(root.alert?.color ?? Theme.accent, 0.18)
        // (a device's picture in the badge's place)
        Image {
            anchors.fill: parent
            visible: !!root.alert?.picture
            source: root.alert?.picture ?? ""
            sourceSize: Qt.size(72, 72)
            fillMode: Image.PreserveAspectFit
            smooth: true
        }
        Icon {
            anchors.centerIn: parent
            visible: !root.alert?.picture
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
                text: NotificationGroups.kindOf(root.alert)
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
    CloseBadge {
        visible: root.hovered
        reach: 3
        onClicked: root.closeRequested()
        Accessible.role: Accessible.Button
        Accessible.name: "Remove " + (root.alert?.title ?? "alert")
    }
}
