import QtQuick
import qs.common
import qs.glass

// One widget on the desktop: liquid glass (drawn in the wallpaper window, so
// it costs no window of its own), the widget inside, and while editing a
// remove badge. Drag it anywhere; on release it snaps to the grid.
Item {
    id: root

    required property var widget // { kind, size, col, row }
    required property int index
    required property var defs // WidgetDefs
    property point origin: Qt.point(32, 54) // grid origin on screen
    property bool editing: false
    signal moved(int index, real x, real y) // dropped at this content position
    signal dragging(int index, real x, real y, bool active) // live, for the snap preview
    signal removeRequested(int index)

    readonly property var def: defs.kind(widget.kind)
    // Items the widget adds to its right-click menu
    function extraMenu() {
        return loader.item?.menuItems?.() ?? [];
    }
    readonly property size px: defs.pixelSize(widget.size)
    readonly property int m: 20 // shadow margin
    readonly property real homeX: origin.x + widget.col * (defs.cell + defs.gap)
    readonly property real homeY: origin.y + widget.row * (defs.cell + defs.gap)

    width: px.width + 2 * m
    height: px.height + 2 * m
    x: homeX - m
    y: homeY - m
    z: drag.active ? 10 : 0
    // Snap back (or into the new cell) with a little spring
    Behavior on x { enabled: !drag.active; SpringAnimation { spring: 5; damping: 0.38; epsilon: 0.3 } }
    Behavior on y { enabled: !drag.active; SpringAnimation { spring: 5; damping: 0.38; epsilon: 0.3 } }
    onXChanged: {
        glass.updatePosition();
        if (drag.active)
            root.dragging(root.index, x + m, y + m, true);
    }
    onYChanged: {
        glass.updatePosition();
        if (drag.active)
            root.dragging(root.index, x + m, y + m, true);
    }
    scale: drag.active ? 1.04 : 1
    Behavior on scale { NumberAnimation { duration: Theme.motion.hover } }

    // Lock and unlock: zooms toward or away from the screen's centre, in
    // turn with the other widgets and icons (LockFx)
    readonly property real fxW: parent?.width ?? 0
    readonly property real fxH: parent?.height ?? 0
    readonly property real fxScale: LockFx.active ? LockFx.itemScale(x + width / 2, y + height / 2, fxW, fxH) : 1
    opacity: shown * (LockFx.active ? LockFx.itemOpacity(x + width / 2, y + height / 2, fxW, fxH) : 1)
    transform: Scale {
        origin.x: root.fxW / 2 - root.x
        origin.y: root.fxH / 2 - root.y
        xScale: root.fxScale
        yScale: root.fxScale
    }

    // Appear: grow in from slightly smaller
    property real shown: 0
    Component.onCompleted: appear.start()
    ParallelAnimation {
        id: appear
        NumberAnimation { target: root; property: "shown"; to: 1; duration: 260; easing.type: Easing.OutCubic }
        NumberAnimation { target: content; property: "scale"; from: 0.9; to: 1; duration: 420; easing.type: Easing.OutBack }
    }

    Glass {
        id: glass
        anchors.fill: parent
        radius: Theme.radius.tile // iOS corners (continuous, like the glass)
        bevel: 20
        shadowMargin: root.m
        forceLiquid: true
    }

    Item {
        id: content
        x: root.m
        y: root.m
        width: root.px.width
        height: root.px.height
        Loader {
            id: loader
            anchors.fill: parent
            anchors.margins: root.def?.bleed ? 0 : 16
            source: root.def ? Qt.resolvedUrl(root.def.file) : ""
            onLoaded: {
                item.size = Qt.binding(() => root.widget.size);
                if (item.widgetId !== undefined)
                    item.widgetId = Qt.binding(() => root.widget.id ?? "");
                if (item.radius !== undefined)
                    item.radius = Theme.radius.tile;
            }
        }
    }

    DragHandler {
        id: drag
        target: root
        dragThreshold: 6
        onActiveChanged: {
            root.dragging(root.index, root.x + root.m, root.y + root.m, active);
            if (!active) {
                root.moved(root.index, root.x + root.m, root.y + root.m);
                // Dragging replaced the position bindings; follow the saved cell again
                root.x = Qt.binding(() => root.homeX - root.m);
                root.y = Qt.binding(() => root.homeY - root.m);
            }
        }
    }

    // Remove badge
    Rectangle {
        visible: root.editing
        x: root.m - 8
        y: root.m - 8
        z: 20
        width: 24
        height: 24
        radius: 12
        color: Theme.dark ? "#3A3A40" : "#E4E4E7"
        border.width: 1
        border.color: Theme.color.separator
        scale: root.editing ? 1 : 0
        Behavior on scale { SpringAnimation { spring: 5; damping: 0.35; epsilon: 0.01 } }
        Rectangle {
            anchors.centerIn: parent
            width: 10
            height: 2
            radius: 1
            color: Theme.color.label
        }
        MouseArea {
            anchors { fill: parent; margins: -6 }
            cursorShape: Qt.PointingHandCursor
            onClicked: root.removeRequested(root.index)
        }
    }
}
