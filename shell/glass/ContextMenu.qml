import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// A glass menu at any point on screen (dock right-click and the like). It's a
// full-screen overlay, so a click anywhere else or Escape closes it. Items:
//   { text, action, enabled?, danger?, confirm? }  or  { separator: true }
// `confirm: "Empty Trash (12 items)"` turns the first click into a second,
// red "are you sure" row; only that one acts. Create it with a Loader and
// clear it from onDismissed.
GlassWindow {
    id: root

    property var items: []
    property real pointX: 0 // screen x the menu centres on
    property real pointY: 0 // screen y of the menu's bottom (opens upward)
    property bool upward: true
    signal dismissed

    property int armed: -1 // index waiting for confirmation
    property int current: -1
    readonly property int menuWidth: Math.max(200, ...items.map(i => (i.confirm && armed >= 0 ? i.confirm.length : (i.text ?? "").length) * 7.5 + 40))

    name: "contextmenu"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    anchors { top: true; bottom: true; left: true; right: true }

    property real progress: 0
    Component.onCompleted: {
        progress = 1;
        body.forceActiveFocus();
    }
    Behavior on progress { SpringAnimation { spring: 5; damping: 0.36; epsilon: 0.005 } }

    function activate(i) {
        const it = items[i];
        if (!it || it.separator || it.enabled === false)
            return;
        if (it.confirm && armed !== i) {
            armed = i;
            return;
        }
        root.dismissed();
        it.action?.();
    }

    ElasticPull {
        id: pull
        target: body
        active: grab.active
        from: grab.point.scenePressPosition
        pointer: grab.point.scenePosition
    }
    // Elastic pull: only while the button is held (press and drag anywhere)
    Item {
        anchors.fill: parent
        z: 1000
        PointHandler { id: grab; acceptedButtons: Qt.LeftButton }
    }
    // A drag that ends outside the panel isn't a click to close it
    readonly property bool dragged: grab.active && Math.hypot(grab.point.scenePosition.x - grab.point.scenePressPosition.x, grab.point.scenePosition.y - grab.point.scenePressPosition.y) > 8
    property bool wasDragged: false
    onDraggedChanged: if (dragged) wasDragged = true
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: root.wasDragged = false
        onClicked: if (!root.wasDragged) root.dismissed()
    }

    Item {
        id: body
        readonly property int m: glass.shadowMargin
        width: root.menuWidth + 2 * m
        height: list.implicitHeight + 10 + 2 * m
        x: Math.max(0, Math.min(root.width - width, root.pointX - width / 2))
        y: root.upward ? root.pointY - height + m : root.pointY - m
        opacity: Math.min(1, root.progress)
        scale: 0.94 + 0.06 * root.progress
        transformOrigin: root.upward ? Item.Bottom : Item.Top
        transform: [pull.translate, pull.stretch]
        onScaleChanged: glass.updatePosition()
        focus: true
        Keys.onEscapePressed: root.dismissed()
        Keys.onDownPressed: root.current = Math.min(root.items.length - 1, root.current + 1)
        Keys.onUpPressed: root.current = Math.max(0, root.current - 1)
        Keys.onReturnPressed: root.activate(root.current)

        Glass {
            id: glass
            anchors.fill: parent
            radius: Theme.radius.menu + 2
            // Follow the elastic pull (the plugin draws the glass from its position)
            Connections {
                target: pull
                function onLxChanged() { glass.updatePosition(); }
                function onLyChanged() { glass.updatePosition(); }
            }
        }
        MouseArea { anchors.fill: parent; anchors.margins: body.m }

        Column {
            id: list
            x: body.m + 5
            y: body.m + 5
            width: root.menuWidth - 10
            Repeater {
                model: root.items
                Item {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool armedRow: root.armed === index
                    readonly property bool enabled_: modelData.enabled !== false
                    readonly property bool hot: (rowMouse.containsMouse || root.current === index) && enabled_
                    width: parent.width
                    height: modelData.separator ? 11 : 28
                    Rectangle {
                        visible: !!row.modelData.separator
                        anchors.centerIn: parent
                        width: parent.width - 18
                        height: 1
                        color: Theme.color.separator
                    }
                    Rectangle {
                        visible: !row.modelData.separator
                        anchors.fill: parent
                        radius: Theme.radius.item + 2
                        color: row.armedRow ? Theme.color.danger : row.hot ? (row.modelData.danger ? Theme.color.danger : Theme.accent) : "transparent"
                        Text {
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.armedRow ? row.modelData.confirm : (row.modelData.text ?? "")
                            color: row.armedRow || row.hot ? "white" : !row.enabled_ ? Theme.color.tertiary : row.modelData.danger ? Theme.color.danger : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: row.armedRow ? Font.DemiBold : Font.Normal }
                        }
                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            enabled: row.enabled_
                            onEntered: root.current = row.index
                            onClicked: root.activate(row.index)
                        }
                    }
                }
            }
        }
    }
}
