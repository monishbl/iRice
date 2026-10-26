import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common

// A glass popover hanging below the menu bar. Shown while Panels says so;
// fades and drops in, closes on a click outside it or Escape. The window is a
// transparent full-screen layer (minus the menu bar, which stays clickable),
// so outside clicks are caught without compositor focus grabs.
// Put content in it directly; it is laid out inside `padding`.
GlassWindow {
    id: root

    required property string popoverName
    // Desired left edge on screen; clamped to stay on screen.
    property real targetX: 0
    property int contentWidth: 320
    // Height of the visible content only (childrenRect counts hidden pages too)
    property int contentHeight: {
        let h = 0;
        for (const c of content.children)
            if (c.visible)
                h = Math.max(h, c.y + c.height);
        return h;
    }
    property int padding: 16
    property real radius: Theme.radius.control + padding // concentric with rows inside
    // No panel: the content floats as its own glass pieces (Control Center's
    // chips in Liquid and Frosted). Clicks between the pieces stay inside.
    property bool bare: false
    // 0..1: darken the panel behind its content (a side panel has the focus)
    property real dim: 0
    property real dimShown: dim
    Behavior on dimShown { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
    default property alias data: content.data
    // Extra floating glass beside the popover (e.g. a Control Center detail
    // page), in window coordinates
    property alias overlay: overlayLayer.data
    // The visible panel, in window coordinates (without the shadow margin)
    readonly property rect panelRect: Qt.rect(body.x + margin, body.y + margin, body.width - 2 * margin, body.height - 2 * margin)
    signal pressedAt(point p) // any press in the window, for closing side panels
    property bool sideOpen: false // Escape closes the side panel first
    signal closeSide

    readonly property bool shown: Panels.isOpen(popoverName, screen)
    readonly property real margin: glass.shadowMargin
    // Starts at 0 so the window animates in when it is created
    // Opens on a spring (a touch of overshoot), closes quickly
    property real progress: 0
    Component.onCompleted: if (shown) openAnim.start()
    onShownChanged: shown ? (closeAnim.stop(), openAnim.start()) : (openAnim.stop(), closeAnim.start())
    SpringAnimation {
        id: openAnim
        target: root
        property: "progress"
        to: 1
        spring: Theme.motion.spring + 0.8
        damping: Theme.motion.damping + 0.06
        epsilon: 0.004
    }
    NumberAnimation {
        id: closeAnim
        target: root
        property: "progress"
        to: 0
        duration: Theme.motion.popover * 0.7
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Theme.motion.exit
    }

    name: popoverName
    visible: progress > 0 && closeGuard.open
    CloseGuard { id: closeGuard; shown: root.shown }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property int bodyWidth: contentWidth + 2 * padding + 2 * margin
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
        PointHandler {
            id: grab
            acceptedButtons: Qt.LeftButton
            onActiveChanged: if (active) root.pressedAt(point.scenePressPosition)
        }
    }
    // A drag that ends outside the panel isn't a click to close it
    readonly property bool dragged: grab.active && Math.hypot(grab.point.scenePosition.x - grab.point.scenePressPosition.x, grab.point.scenePosition.y - grab.point.scenePressPosition.y) > 8
    property bool wasDragged: false
    onDraggedChanged: if (dragged) wasDragged = true
    readonly property int bodyHeight: contentHeight + 2 * padding + 2 * margin

    // The whole screen takes input while open: outside clicks close; clicks on
    // the menu bar strip go to the item there (another menu opens, the same
    // one closes), and sliding along the bar switches between menus.
    MouseArea {
        id: catcher
        anchors.fill: parent
        enabled: root.shown
        hoverEnabled: true
        onPressed: root.wasDragged = false
        onClicked: m => {
            if (root.wasDragged)
                return;
            const item = m.y < 30 ? Panels.barItemAt(root.screen, m.x) : null;
            if (item)
                item.clicked(m);
            else
                Panels.close();
        }
        onPositionChanged: m => {
            if (m.y >= 30 || pressed)
                return;
            const item = Panels.barItemAt(root.screen, m.x);
            if (item && item.menu !== "" && item.menu !== root.popoverName && !Panels.isOpen(item.menu, root.screen))
                item.clicked(m);
        }
    }

    Item {
        id: body
        width: root.bodyWidth
        height: root.bodyHeight
        x: Math.max(0, Math.min(root.targetX - root.margin, root.width - width))
        y: 30 + 4 - root.margin + (1 - root.progress) * -8
        onXChanged: glass.updatePosition()
        opacity: Math.min(1, root.progress)
        transform: [pull.translate, pull.stretch]
        focus: root.shown
        Keys.onEscapePressed: root.sideOpen ? root.closeSide() : Panels.close()

        Glass {
            id: glass
            anchors.fill: parent
            visible: !root.bare
            radius: root.radius
            Connections {
                target: body
                function onYChanged() { glass.updatePosition(); }
            }
            Connections {
                target: pull
                function onLxChanged() { glass.updatePosition(); }
                function onLyChanged() { glass.updatePosition(); }
            }
        }

        Rectangle {
            x: root.margin
            y: root.margin
            width: parent.width - 2 * root.margin
            height: parent.height - 2 * root.margin
            radius: root.radius
            color: "black"
            opacity: 0.14 * root.dimShown
            visible: opacity > 0 && !root.bare
        }
        MouseArea {
            visible: root.bare
            x: root.margin
            y: root.margin
            width: parent.width - 2 * root.margin
            height: parent.height - 2 * root.margin
        }
        Item {
            id: content
            x: root.margin + root.padding
            y: root.margin + root.padding
            width: root.contentWidth
            height: root.contentHeight
        }
    }
    Item {
        id: overlayLayer
        anchors.fill: parent
        opacity: Math.min(1, root.progress)
    }
}
