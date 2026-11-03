import QtQuick
import qs.common

// A glass panel's give (Popover, Dialog, ContextMenu): a press anywhere in
// the window, above everything in it, lets `target` follow the drag like
// rubber while the button is held (ElasticPull), and remembers a drag so one
// that ends outside the panel isn't a click to close it.
//   ElasticGrab { id: grab; target: body }   body.transform: [grab.pull.translate, grab.pull.stretch]
//   MouseArea { onPressed: grab.wasDragged = false; onClicked: if (!grab.wasDragged) close() }
Item {
    id: root

    property Item target
    readonly property alias pull: pull
    readonly property bool dragged: handler.active && Math.hypot(handler.point.scenePosition.x - handler.point.scenePressPosition.x, handler.point.scenePosition.y - handler.point.scenePressPosition.y) > 8
    property bool wasDragged: false
    signal grabbed(point at) // any press in the window

    anchors.fill: parent
    z: 1000
    onDraggedChanged: if (dragged) wasDragged = true

    PointHandler {
        id: handler
        acceptedButtons: Qt.LeftButton
        onActiveChanged: if (active) root.grabbed(handler.point.scenePressPosition)
    }
    ElasticPull {
        id: pull
        target: root.target
        active: handler.active && Panels.holdPull === 0 && !Panels.ccEditing
        from: handler.point.scenePressPosition
        pointer: handler.point.scenePosition
    }
}
