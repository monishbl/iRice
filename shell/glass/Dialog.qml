import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// A centred glass dialog over a light scrim. Escape or a click outside closes
// it. Content goes inside, laid out within `padding`.
GlassWindow {
    id: root

    required property string dialogName
    property int contentWidth: 360
    property int contentHeight: content.childrenRect.height
    property int padding: 20
    default property alias data: content.data
    signal tabPressed

    readonly property bool shown: Panels.isOpen(dialogName, screen)
    readonly property int m: 24
    // Starts at 0 so the window animates in when it is created
    // Opens on a spring, closes quickly
    property real progress: 0
    Component.onCompleted: if (shown) openAnim.start()
    onShownChanged: shown ? (closeAnim.stop(), openAnim.start()) : (openAnim.stop(), closeAnim.start())
    SpringAnimation {
        id: openAnim
        target: root
        property: "progress"
        to: 1
        spring: Theme.motion.spring + 0.4
        damping: Theme.motion.damping + 0.08
        epsilon: 0.004
    }
    NumberAnimation {
        id: closeAnim
        target: root
        property: "progress"
        to: 0
        duration: Theme.motion.panel * 0.6
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Theme.motion.exit
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

    name: dialogName
    visible: progress > 0 && closeGuard.open
    CloseGuard { id: closeGuard; shown: root.shown }
    anchors { top: true; bottom: true; left: true; right: true }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0, 0, 0, 0.25 * root.progress)
        MouseArea {
            anchors.fill: parent
            onPressed: root.wasDragged = false
            onClicked: if (!root.wasDragged) Panels.close()
        }
    }

    Item {
        id: body
        width: root.contentWidth + 2 * root.padding + 2 * root.m
        height: root.contentHeight + 2 * root.padding + 2 * root.m
        x: (parent.width - width) / 2
        y: (parent.height - height) / 2 - 40
        opacity: Math.min(1, root.progress)
        scale: 0.96 + 0.04 * root.progress
        transform: [pull.translate, pull.stretch]
        focus: root.shown
        Keys.onEscapePressed: Panels.close()
        Keys.onTabPressed: root.tabPressed()

        Glass {
            id: glass
            anchors.fill: parent
            radius: 28
            bevel: 22
            shadowMargin: root.m
            Connections {
                target: body
                function onScaleChanged() { glass.updatePosition(); }
            }
            Connections {
                target: pull
                function onLxChanged() { glass.updatePosition(); }
                function onLyChanged() { glass.updatePosition(); }
            }
        }
        // Swallow clicks on the dialog so they don't reach the scrim
        MouseArea {
            anchors.fill: parent
            anchors.margins: root.m
        }

        Item {
            id: content
            x: root.m + root.padding
            y: root.m + root.padding
            width: root.contentWidth
            height: root.contentHeight
        }
    }
}
