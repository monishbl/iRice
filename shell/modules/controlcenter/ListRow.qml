import QtQuick
import qs.common

// A selectable row in a Control Center page: leading icon, title, status
// text, trailing icons. `busy` shows a spinner in place of the trailing icon.
Rectangle {
    id: root

    property string icon
    property bool iconFill: false
    property bool highlighted: false
    property string title
    property string status
    property string trailingIcon
    property bool busy: false
    signal clicked
    signal menuRequested // right-click (or press and hold): the row's menu

    // Keyboard: Tab or the arrows to it, Space or Enter acts, the Menu key
    // (or Shift+F10) opens its menu
    activeFocusOnTab: visible
    Keys.onPressed: event => {
        const k = event.key;
        if ((k === Qt.Key_Space || k === Qt.Key_Return || k === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.clicked();
        } else if (k === Qt.Key_Menu || (k === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
            root.menuRequested();
        } else if (k === Qt.Key_Down || k === Qt.Key_Up) {
            // to the next or previous row
            const next = root.nextItemInFocusChain(k === Qt.Key_Down);
            if (next)
                next.forceActiveFocus(k === Qt.Key_Down ? Qt.TabFocusReason : Qt.BacktabFocusReason);
        } else {
            return;
        }
        event.accepted = true;
    }
    Accessible.role: Accessible.ListItem
    Accessible.name: title
    Accessible.description: status
    Accessible.selected: highlighted
    Accessible.onPressAction: root.clicked()
    FocusRing {
        radius: root.radius + 4
    }

    width: parent?.width ?? 0
    height: 40
    radius: Theme.radius.control
    color: mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fill : "transparent"

    Rectangle {
        id: badge
        x: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 28
        height: 28
        radius: 14
        color: root.highlighted ? Theme.accent : Theme.color.toggleOff
        Icon {
            anchors.centerIn: parent
            name: root.icon
            fill: root.iconFill || root.highlighted
            size: 15
            color: root.highlighted ? Theme.color.onAccent : Theme.color.label
        }
    }

    Column {
        anchors { left: badge.right; leftMargin: 10; right: trailing.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: root.highlighted ? Font.DemiBold : Font.Normal }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.status
            elide: Text.ElideRight
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    Icon {
        id: trailing
        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
        name: root.busy ? "circle-notch" : root.trailingIcon
        size: 14
        color: Theme.color.secondary
        RotationAnimation on rotation { running: root.busy; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
        onNameChanged: if (!root.busy) rotation = 0
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        pressAndHoldInterval: 550
        property bool held: false // the press became a press and hold: no click
        onPressed: held = false
        onClicked: m => {
            if (m.button === Qt.RightButton)
                root.menuRequested();
            else if (!held)
                root.clicked();
        }
        onPressAndHold: m => {
            if (m.button === Qt.LeftButton) {
                held = true;
                root.menuRequested();
            }
        }
    }
}
