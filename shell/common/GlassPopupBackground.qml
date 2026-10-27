import QtQuick
import QtQuick.Effects

// Frosted glass for pop-up menus and lists inside a window (a QQC.Popup's
// background, the dock's name label): what's behind it in the window is
// blurred and tinted with the glass material, with a light rim, like the menu
// bar's menus. Popups draw in the window's overlay, above its content, so the
// content under them can be copied and blurred here (live while it's shown).
// In layer windows the compositor already blurs what's behind the window, so
// `blurContent: false` keeps just the tint.
Item {
    id: root

    property bool blurContent: true
    property real radius: 12 // continuous (squircle) corners, like the glass
    // As opaque as the material asks, never so clear that text gets hard to read
    readonly property color color: Theme.glass.tier === "solid" ? Theme.glass.solid : Theme.dark ? Qt.rgba(0.11, 0.11, 0.13, 0.55) : Qt.rgba(0.97, 0.97, 0.99, 0.62)

    // What gets blurred: the window's content item that's under the popup.
    // Not the content item itself: popups draw in its overlay, so copying
    // all of it would copy this popup too (a feedback loop of smeared copies).
    readonly property Item content: Window.contentItem
    property Item page: null
    readonly property bool blurring: blurContent && visible && page !== null && Theme.glass.tier !== "solid"
    readonly property int pad: 24 // blur reaches this far in from outside the edges
    // Where this sits over `page`, padded (popups don't move while open)
    property rect area: Qt.rect(0, 0, 0, 0)
    function place() {
        if (!content)
            return;
        const p = mapToItem(content, 0, 0);
        let best = null;
        for (const c of content.children) {
            if (!c.visible || String(c).startsWith("QQuickOverlay"))
                continue;
            if (p.x + width / 2 >= c.x && p.x + width / 2 <= c.x + c.width && p.y >= c.y && p.y <= c.y + c.height)
                best = c;
        }
        page = best;
        if (!best)
            return;
        const q = mapToItem(best, 0, 0);
        area = Qt.rect(q.x - pad, q.y - pad, width + 2 * pad, height + 2 * pad);
    }
    onVisibleChanged: if (visible) Qt.callLater(place)
    onWidthChanged: place()
    onHeightChanged: place()

    ShaderEffectSource {
        id: behind
        visible: false
        sourceItem: root.blurring ? root.page : null
        sourceRect: root.area
        live: root.blurring
        width: root.area.width
        height: root.area.height
    }
    // The rounded shape inside the padding, so the blur doesn't spill past the corners
    Item {
        id: shape
        visible: false
        width: root.area.width
        height: root.area.height
        layer.enabled: root.blurring
        Squircle {
            x: root.pad
            y: root.pad
            width: root.width
            height: root.height
            radius: root.radius
            color: "white"
        }
    }
    MultiEffect {
        z: -1
        visible: root.blurring
        x: -root.pad
        y: -root.pad
        width: root.area.width
        height: root.area.height
        source: behind
        autoPaddingEnabled: false
        blurEnabled: true
        blur: 1
        blurMax: 40
        saturation: 0.3
        maskEnabled: true
        maskSource: shape
    }

    // The tint and its hairline edge, in the same shape
    Squircle {
        anchors.fill: parent
        radius: root.radius
        color: root.color
        borderWidth: 1
        borderColor: Theme.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.1)
    }

    // Light catching the top edge (a line, not a gradient: translucent
    // gradients on rounded rectangles draw as mismatched triangles)
    Rectangle {
        anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: root.radius * 1.28; rightMargin: root.radius * 1.28; topMargin: 1 }
        height: 1
        color: Qt.rgba(1, 1, 1, Theme.dark ? 0.12 : 0.6)
    }
}
