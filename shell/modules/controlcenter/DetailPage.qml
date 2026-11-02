import QtQuick
import qs.common

// A Control Center sub-page: back button, title, optional switch, then content.
// Material (Quick Settings' pages): a back arrow and the title at 22 px, and
// the switch as Android's main switch bar under them (a pill that fills
// with primaryContainer when on).
Column {
    id: root

    property string title
    property bool showBack: true // false when shown as a menu bar menu
    property bool hasSwitch: false
    property bool switchOn: false
    property bool hasRefresh: false // a refresh button (lists that change on their own)
    property string switchLabel: "Use " + title // (Material's switch bar)
    signal back
    signal switchToggled
    signal refreshRequested
    // The pointer is over the page: lists hold still so rows don't move under it
    readonly property bool hovered: pageHover.hovered
    HoverHandler { id: pageHover }
    default property alias content: body.data

    spacing: 10

    Item {
        width: parent.width
        height: Theme.material ? 48 : 30
        visible: root.showBack || root.title !== "" || root.hasSwitch

        Rectangle {
            id: backButton
            visible: root.showBack && !Theme.material
            width: root.showBack ? 30 : 0
            height: 30
            radius: 15
            color: backMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
            Icon { anchors.centerIn: parent; name: "caret-left"; bold: true; size: 14 }
            MouseArea {
                id: backMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.back()
            }
        }
        Text {
            visible: !Theme.material
            anchors { left: backButton.right; leftMargin: root.showBack ? 10 : 2; verticalCenter: parent.verticalCenter }
            text: root.title
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
        }
        Rectangle {
            id: refreshButton
            visible: root.hasRefresh && !Theme.material
            anchors { right: root.hasSwitch ? pageSwitch.left : parent.right; rightMargin: root.hasSwitch ? 10 : 0; verticalCenter: parent.verticalCenter }
            width: 28
            height: 28
            radius: 14
            color: refreshMouse.pressed ? Theme.color.fillPressed : refreshMouse.containsMouse ? Theme.color.fillHover : "transparent"
            Icon {
                id: refreshIcon
                anchors.centerIn: parent
                name: "arrow-clockwise"
                size: 14
                color: Theme.color.secondary
                RotationAnimation on rotation { id: spin; running: false; from: 0; to: 360; duration: 500; easing.type: Easing.OutCubic }
            }
            MouseArea {
                id: refreshMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    spin.restart();
                    root.refreshRequested();
                }
            }
        }
        Switch {
            id: pageSwitch
            visible: root.hasSwitch && !Theme.material
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            checked: root.switchOn
            onToggled: root.switchToggled()
        }

        // Material: back arrow, title, refresh
        Loader {
            anchors.fill: parent
            active: Theme.material
            sourceComponent: Item {
                IconButton {
                    id: mBack
                    visible: root.showBack
                    anchors.verticalCenter: parent.verticalCenter
                    width: visible ? 40 : 0
                    height: 40
                    icon: "caret-left"
                    iconSize: 22
                    iconColor: Theme.m3.onSurface
                    tooltip: "Back"
                    onClicked: root.back()
                }
                Text {
                    anchors { left: mBack.right; leftMargin: root.showBack ? 8 : 4; right: mRefresh.visible ? mRefresh.left : parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    text: root.title
                    elide: Text.ElideRight
                    color: Theme.m3.onSurface
                    font { family: Theme.font.family; pixelSize: 22 }
                }
                IconButton {
                    id: mRefresh
                    visible: root.hasRefresh
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    width: 40
                    height: 40
                    icon: "arrow-clockwise"
                    iconSize: 20
                    iconColor: Theme.m3.onSurfaceVariant
                    tooltip: "Refresh"
                    RotationAnimation on rotation { id: mSpin; running: false; from: 0; to: 360; duration: 500; easing.type: Easing.OutCubic }
                    onClicked: {
                        mSpin.restart();
                        root.refreshRequested();
                    }
                }
            }
        }
    }

    // Material: the main switch bar, Android's: the whole bar switches
    Loader {
        active: Theme.material && root.hasSwitch
        visible: active
        width: parent.width
        sourceComponent: Rectangle {
            height: 56
            radius: 28
            readonly property color ink: root.switchOn ? Theme.m3.onPrimaryContainer : Theme.m3.onSurface
            color: Qt.tint(root.switchOn ? Theme.m3.primaryContainer : Theme.m3.surfaceContainerHighest, Qt.alpha(ink, barMouse.pressed ? 0.1 : barMouse.containsMouse ? 0.08 : 0))
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            Text {
                anchors { left: parent.left; leftMargin: 24; right: barSwitch.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                text: root.switchLabel
                elide: Text.ElideRight
                color: parent.ink
                font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
            }
            MouseArea {
                id: barMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.switchToggled()
            }
            Switch {
                id: barSwitch
                anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                checked: root.switchOn
                accessibleName: root.switchLabel
                onToggled: root.switchToggled()
            }
        }
    }

    Column {
        id: body
        width: parent.width
        spacing: 2
    }
}
