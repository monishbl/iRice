import QtQuick
import QtQuick as QQ
import qs.common

// A page's less used settings, folded away at its end: "More options" opens
// them. Closed each time the page opens; a search for one of its rows opens
// it (Settings' reveal() looks for `moreOptions`).
//   MoreOptions { Group { … } Group { … } }
Column {
    id: root

    readonly property bool moreOptions: true
    property bool expanded: false
    default property alias content: body.data

    width: parent?.width ?? 0
    spacing: 22

    Item {
        width: root.width
        height: 44
        activeFocusOnTab: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                root.expanded = !root.expanded;
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: root.expanded ? "Fewer options" : "More options"
        Accessible.onPressAction: root.expanded = !root.expanded

        // Glass: a text button in the accent; Material: a tonal pill
        Rectangle {
            id: pill
            anchors.verticalCenter: parent.verticalCenter
            x: Theme.material ? 0 : -4
            width: labelRow.implicitWidth + (Theme.material ? 32 : 16)
            height: Theme.material ? 40 : 30
            radius: height / 2
            color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0)) : mouse.containsMouse ? Theme.color.fill : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            QQ.Row {
                id: labelRow
                anchors.centerIn: parent
                spacing: 6
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.expanded ? "Fewer options" : "More options"
                    color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.accentInk
                    font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: Font.Medium }
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "caret-down"
                    bold: true
                    size: 12
                    rotation: root.expanded ? 180 : 0
                    Behavior on rotation { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                    color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.accentInk
                }
            }
            MouseArea {
                id: mouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.expanded = !root.expanded
            }
        }
        FocusRing {
            anchors.fill: pill
            radius: pill.radius + 4
        }
    }

    Column {
        id: body
        visible: root.expanded
        width: root.width
        spacing: 22
    }
}
