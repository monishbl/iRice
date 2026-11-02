import QtQuick
import qs.common

// A selectable row in a Control Center page: leading icon, title, status
// text, trailing icons. `busy` shows a spinner in place of the trailing icon.
// `actionIcon`: a small button of its own before it (actionClicked).
// Material: an item of Material 3 Expressive's segmented list. Rows that
// follow one another are 2 px apart with small inner corners and the
// group's ends round (it finds its neighbours itself); the highlighted row
// (connected, chosen) fills with primaryContainer, and hover, press and
// highlight round its inner corners more.
Squircle {
    id: root

    property string icon
    property string picture: "" // a device's picture (Devices.pictureFor) in the badge's place
    property bool iconFill: false
    property bool highlighted: false
    property string title
    property string status
    property string trailingIcon
    property bool busy: false
    property string actionIcon
    property string actionTip
    signal clicked
    signal actionClicked
    signal menuRequested // right-click (or press and hold): the row's menu
    readonly property bool isListRow: true

    // ── Material: its place in the group ────────────────────────────────
    // Another row right before (dir -1) or after (+1) it, skipping hidden
    // items and Repeaters. A row that's the first or last thing in a
    // Repeater's delegate (a Wi-Fi network with its password prompt) looks
    // beside that delegate too.
    readonly property bool roundTop: Theme.material && !rowBeside(-1)
    readonly property bool roundBottom: Theme.material && !rowBeside(1)
    function edgeIsRow(it, dir) {
        if (it.isListRow === true)
            return true;
        if (it.modelData === undefined)
            return false;
        const kids = it.children;
        for (let i = dir > 0 ? 0 : kids.length - 1; i >= 0 && i < kids.length; i += dir) {
            const k = kids[i];
            if (!k.visible || typeof k.itemAt === "function" || k.height <= 0)
                continue;
            return k.isListRow === true;
        }
        return false;
    }
    function rowBeside(dir) {
        let item = root, box = root.parent;
        for (let depth = 0; box && depth < 2; depth++) {
            const kids = box.children;
            let i = -1;
            for (let j = 0; j < kids.length; j++)
                if (kids[j] === item)
                    i = j;
            for (i += dir; i >= 0 && i < kids.length; i += dir) {
                const k = kids[i];
                if (!k.visible || typeof k.itemAt === "function" || k.height <= 0)
                    continue;
                return edgeIsRow(k, dir);
            }
            // At the edge of a Repeater's delegate: look beside the delegate
            if (box.modelData === undefined)
                return false;
            item = box;
            box = box.parent;
        }
        return false;
    }

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
    height: Theme.material ? (status !== "" ? 56 : 48) : 40
    radius: Theme.radius.control
    color: Theme.material ? "transparent" : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fill : "transparent"

    // Material: the segment
    Loader {
        anchors.fill: parent
        active: Theme.material
        sourceComponent: Rectangle {
            readonly property color ink: root.highlighted ? Theme.m3.onPrimaryContainer : Theme.m3.onSurface
            property real inner: root.highlighted || mouse.pressed ? 16 : mouse.containsMouse ? 12 : 4
            Behavior on inner {
                SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.2 }
            }
            topLeftRadius: root.roundTop ? 20 : inner
            topRightRadius: root.roundTop ? 20 : inner
            bottomLeftRadius: root.roundBottom ? 20 : inner
            bottomRightRadius: root.roundBottom ? 20 : inner
            color: Qt.tint(root.highlighted ? Theme.m3.primaryContainer : Theme.m3.surfaceContainerHigh, Qt.alpha(ink, mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0))
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }
    }

    Rectangle {
        id: badge
        x: Theme.material ? 10 : 6
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.material ? 36 : 28
        height: width
        radius: width / 2
        color: root.picture !== "" ? "transparent" : root.highlighted ? Theme.accent : Theme.color.toggleOff
        Image {
            anchors.fill: parent
            visible: root.picture !== ""
            source: root.picture
            sourceSize: Theme.material ? Qt.size(72, 72) : Qt.size(56, 56)
            fillMode: Image.PreserveAspectFit
            smooth: true
        }
        // (with a picture, connected is an accent dot at its corner)
        Rectangle {
            visible: root.picture !== "" && root.highlighted
            anchors { right: parent.right; bottom: parent.bottom; rightMargin: -2; bottomMargin: -2 }
            width: 10
            height: 10
            radius: 5
            color: Theme.accent
            border.width: 2
            border.color: Theme.glass.solid
        }
        Icon {
            anchors.centerIn: parent
            visible: root.picture === ""
            name: root.icon
            fill: root.iconFill || root.highlighted
            size: Theme.material ? 18 : 15
            color: root.highlighted ? Theme.color.onAccent : Theme.color.label
        }
    }

    Column {
        anchors { left: badge.right; leftMargin: Theme.material ? 14 : 10; right: action.visible ? action.left : trailing.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: Theme.material && root.highlighted ? Theme.m3.onPrimaryContainer : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: root.highlighted ? (Theme.material ? Font.Medium : Font.DemiBold) : Font.Normal }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.status
            elide: Text.ElideRight
            color: Theme.material && root.highlighted ? Qt.alpha(Theme.m3.onPrimaryContainer, 0.8) : Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.material ? 12 : Theme.font.caption }
        }
    }

    Icon {
        id: trailing
        anchors { right: parent.right; rightMargin: Theme.material ? 16 : 10; verticalCenter: parent.verticalCenter }
        // (Material: the loading indicator draws busy instead)
        name: root.busy && !Theme.material ? "circle-notch" : root.busy ? "" : root.trailingIcon
        width: Theme.material && root.busy ? 22 : implicitWidth
        size: Theme.material ? 18 : 14
        color: Theme.material && root.highlighted ? Theme.m3.onPrimaryContainer : Theme.color.secondary
        RotationAnimation on rotation { running: root.busy && !Theme.material; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
        onNameChanged: if (!root.busy) rotation = 0
    }
    Loader {
        active: Theme.material && root.busy
        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
        sourceComponent: Spinner {
            size: 22
        }
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

    IconButton {
        id: action
        visible: root.actionIcon !== ""
        anchors { right: trailing.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
        icon: root.actionIcon
        iconSize: 15
        tooltip: root.actionTip
        onClicked: root.actionClicked()
    }
}
