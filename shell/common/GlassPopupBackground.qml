import QtQuick
import QtQuick.Effects

// Frosted glass for pop-up menus and lists inside a window (a QQC.Popup's
// background, the dock's name label): what's behind it in the window is
// blurred and tinted with the glass material, with a light rim, like the menu
// bar's menus. Popups draw in the window's overlay, above its content, so the
// content under them can be copied and blurred here (live while it's shown).
// In layer windows the compositor already blurs what's behind the window, so
// `blurContent: false` keeps just the tint.
Rectangle {
    id: root

    property bool blurContent: true
    radius: 12
    // As opaque as the material asks, never so clear that text gets hard to read
    color: Theme.glass.tier === "solid" ? Theme.glass.solid : Theme.dark ? Qt.rgba(0.11, 0.11, 0.13, 0.55) : Qt.rgba(0.97, 0.97, 0.99, 0.62)
    border.width: 1
    border.color: Theme.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.1)

    readonly property Item content: Window.contentItem
    readonly property bool blurring: blurContent && visible && content !== null && Theme.glass.tier !== "solid"
    // Where this sits over the window's content (popups don't move while open)
    property rect area: Qt.rect(0, 0, 0, 0)
    function place() {
        if (!content)
            return;
        const p = mapToItem(content, 0, 0);
        area = Qt.rect(p.x, p.y, width, height);
    }
    onVisibleChanged: if (visible) Qt.callLater(place)
    onWidthChanged: place()
    onHeightChanged: place()

    ShaderEffectSource {
        id: behind
        visible: false
        sourceItem: root.blurring ? root.content : null
        sourceRect: root.area
        live: root.blurring
        width: root.width
        height: root.height
    }
    // The rounded shape, so the blur doesn't spill past the corners
    Rectangle {
        id: shape
        visible: false
        width: root.width
        height: root.height
        radius: root.radius
        layer.enabled: root.blurring
    }
    MultiEffect {
        z: -1
        visible: root.blurring
        anchors.fill: parent
        source: behind
        blurEnabled: true
        blur: 1
        blurMax: 40
        saturation: 0.3
        maskEnabled: true
        maskSource: shape
    }

    // Light catching the top edge (a line, not a gradient: translucent
    // gradients on rounded rectangles draw as mismatched triangles)
    Rectangle {
        anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: parent.radius; rightMargin: parent.radius; topMargin: 1 }
        height: 1
        color: Qt.rgba(1, 1, 1, Theme.dark ? 0.12 : 0.6)
    }
}
