import QtQuick
import qs.common
import qs.services

// One row of the shelf: a thumbnail or icon, a name and where it is. Drag it
// out to another app (a native drag, with the files' URIs and text). It stays
// on the shelf, so it can go to more than one place: Qt can't tell on Wayland
// whether the drop was taken (it reports none either way).
Item {
    id: root

    property var entries: [] // the shelf items this row carries
    property string label
    property string detail
    property string glyph: "file"
    property string image // a thumbnail's URL, if any
    property bool removable: false
    property int elide: Text.ElideMiddle // (file names keep their extension)
    signal remove

    HoverHandler { id: hover }
    Rectangle {
        anchors.fill: parent
        radius: 10
        color: grab.pressed ? Theme.color.fillPressed : grab.containsMouse ? Theme.color.fill : "transparent"
    }

    Rectangle {
        id: thumb
        x: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 32
        height: 32
        radius: 6
        color: Theme.color.fill
        clip: true
        Image {
            anchors.fill: parent
            visible: root.image !== ""
            source: root.image
            sourceSize: Qt.size(64, 64)
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }
        Icon {
            anchors.centerIn: parent
            visible: root.image === ""
            name: root.glyph
            size: 18
            color: Theme.color.secondary
        }
    }

    Column {
        anchors { left: thumb.right; leftMargin: 10; right: close.visible ? close.left : parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
        Text {
            width: parent.width
            text: root.label
            elide: root.elide
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.detail
            elide: Text.ElideMiddle
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    // The drag itself (Qt's own: other apps see an ordinary file drag)
    Item {
        id: proxy
        width: 1
        height: 1
        Drag.active: grab.drag.active
        Drag.dragType: Drag.Automatic
        Drag.supportedActions: Qt.CopyAction | Qt.MoveAction | Qt.LinkAction
        Drag.proposedAction: Qt.CopyAction
        Drag.mimeData: Shelf.mimeFor(root.entries)
        Drag.onDragFinished: {
            proxy.x = 0;
            proxy.y = 0;
        }
    }
    MouseArea {
        id: grab
        anchors.fill: parent
        hoverEnabled: true
        drag.target: proxy
        cursorShape: Qt.OpenHandCursor
    }

    IconButton {
        id: close
        visible: root.removable && hover.hovered
        anchors { right: parent.right; rightMargin: 4; verticalCenter: parent.verticalCenter }
        icon: "x"
        iconSize: 13
        tooltip: "Remove from the shelf"
        onClicked: root.remove()
    }
}
