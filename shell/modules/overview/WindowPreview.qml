import QtQuick
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.services

// One window inside an Overview space: a picture of its content with the
// app icon as a small badge. While the picture loads, or if it can't be
// taken, the icon sits in the middle instead (with the app's name when the
// capture failed and there is room).
// Nothing is captured while `capture` is false (the Overview is closed);
// `live` keeps the picture updating, otherwise it is taken once.
Item {
    id: root

    required property var toplevel
    property bool capture: false
    property bool live: false
    property bool hovered: false
    property real radius: 4

    readonly property var o: toplevel?.lastIpcObject ?? ({})
    readonly property string cls: o.class ?? ""
    readonly property bool loaded: view.hasContent
    // No picture is coming: no Wayland handle for the window, or the capture stopped
    property bool failed: false
    readonly property bool noSource: !(toplevel?.wayland)

    // Take a fresh picture (the Overview reopened before its window unloaded)
    function recapture() {
        failed = false;
        if (!live && view.captureSource)
            view.captureFrame();
    }
    onCaptureChanged: if (capture) failed = false

    ClippingRectangle {
        anchors.fill: parent
        radius: root.radius
        color: Theme.dark ? "#2C2C31" : "#FFFFFF"

        ScreencopyView {
            id: view
            anchors.fill: parent
            captureSource: root.capture ? (root.toplevel?.wayland ?? null) : null
            live: root.live
            // Matches the miniature, so frames are no bigger than drawn
            constraintSize: Qt.size(Math.max(1, Math.ceil(root.width)), Math.max(1, Math.ceil(root.height)))
            opacity: root.loaded ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
            onStopped: if (root.capture && !hasContent) root.failed = true
        }
    }

    // Loading or failed: the icon in the middle
    Column {
        visible: !root.loaded
        anchors.centerIn: parent
        width: parent.width - 12
        spacing: 4
        readonly property bool roomy: root.height >= 56 && root.width >= 72
        AppIcon {
            anchors.horizontalCenter: parent.horizontalCenter
            size: Math.round(Math.max(12, Math.min(32, root.height * (parent.roomy ? 0.4 : 0.7))))
            source: Apps.iconFor(root.cls)
        }
        Text {
            visible: parent.roomy && (root.failed || root.noSource)
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: Apps.entryFor(root.cls)?.name || root.cls
            elide: Text.ElideRight
            color: Theme.dark ? "#EDEDEF" : "#18181B"
            font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium }
        }
    }

    // Loaded: the icon as a badge in the corner
    AppIcon {
        readonly property bool fits: root.width >= 40 && root.height >= 32
        visible: root.loaded && fits
        anchors { right: parent.right; bottom: parent.bottom; margins: 4 }
        size: Math.round(Math.max(12, Math.min(20, root.height * 0.22)))
        source: Apps.iconFor(root.cls)
    }

    // Outline; the accent shows which window a click focuses
    Rectangle {
        anchors.fill: parent
        radius: root.radius
        color: "transparent"
        border.width: root.hovered ? 2 : 1
        border.color: root.hovered ? Theme.accent : (Theme.dark ? "#36363C" : "#E4E4E7")
    }
}
