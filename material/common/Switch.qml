import QtQuick

// Toggle switch: macOS's, or Material 3 Expressive's in the Material look (a
// 52×32 outlined track; the handle grows 16 → 24 when on, with a check, and
// 28 while pressed, moving on Material's fast spring).
Item {
    id: root

    property bool checked: false
    property string accessibleName // what it switches (a settings Row names it after its label)
    signal toggled

    implicitWidth: Theme.material ? 52 : 38
    implicitHeight: Theme.material ? 32 : 22
    opacity: enabled ? 1 : 0.4 // (disabled: can't be switched here, the row says why)
    activeFocusOnTab: true
    Keys.onPressed: event => {
        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.toggled();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.CheckBox
    Accessible.name: accessibleName
    Accessible.checkable: true
    Accessible.checked: checked
    Accessible.onToggleAction: if (enabled) root.toggled()
    Accessible.onPressAction: if (enabled) root.toggled()
    FocusRing {
        radius: height / 2
    }


    // ── Material ──
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Rectangle {
            id: track
            radius: height / 2
            color: root.checked ? Theme.m3.primary : Theme.m3.surfaceContainerHighest
            border.width: root.checked ? 0 : 2
            border.color: Theme.m3.outline
            Behavior on color { ColorAnimation { duration: 150 } }
            // state layer around the handle on hover and press
            Rectangle {
                width: 40
                height: 40
                radius: 20
                anchors.centerIn: handle
                color: root.checked ? Theme.m3.primary : Theme.m3.onSurface
                opacity: area.pressed ? 0.1 : area.containsMouse ? 0.08 : 0
            }
            Rectangle {
                id: handle
                property real size: area.pressed ? 28 : root.checked ? 24 : 16
                width: size
                height: size
                radius: size / 2
                anchors.verticalCenter: parent.verticalCenter
                x: root.checked ? track.width - 16 - size / 2 : 16 - size / 2
                color: root.checked ? Theme.m3.onPrimary : area.containsMouse || area.pressed ? Theme.m3.onSurfaceVariant : Theme.m3.outline
                // (Material's fast spatial spring: ζ 0.6, k 800)
                Behavior on x { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.2 } }
                Behavior on size { SpringAnimation { spring: 12.8; damping: 0.54; epsilon: 0.2 } }
                Behavior on color { ColorAnimation { duration: 150 } }
                Icon {
                    anchors.centerIn: parent
                    visible: root.checked
                    name: "check"
                    size: 13
                    bold: true
                    color: Theme.m3.onPrimaryContainer
                }
            }
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: Theme.material
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
