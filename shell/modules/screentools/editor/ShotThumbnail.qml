import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import qs.common
import qs.glass

// The floating thumbnail after a screenshot, bottom right. It slides in,
// waits 5 s (paused while hovered), then slides out. Click to mark it up,
// drag it out to drop the file into an app, swipe right (or the close
// button) to dismiss. After Delete it becomes the Undo toast instead.
GlassWindow {
    id: win

    required property var session
    readonly property bool deleted: session.thumb === "deleted"
    readonly property string file: session.thumbFile
    readonly property int m: 24 // room for the glass shadow
    readonly property int edge: 24 // gap to the screen edge (the window sits flush)

    name: "shotThumbnail"
    screen: session.screen ?? Quickshell.screens[0]
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    anchors { bottom: true; right: true }
    margins { bottom: edge - m; right: edge - m }

    // Fit the image in 200 x 200, never thinner than 72
    readonly property real aspect: preview.status === Image.Ready && preview.implicitHeight > 0 ? preview.implicitWidth / preview.implicitHeight : 16 / 10
    readonly property int thumbW: Math.round(aspect >= 1 ? 200 : Math.max(72, 200 * aspect))
    readonly property int thumbH: Math.round(aspect >= 1 ? Math.max(72, 200 / aspect) : 200)
    readonly property int cardW: deleted ? toastRow.implicitWidth + 24 : thumbW + 12
    readonly property int cardH: deleted ? 44 : thumbH + 12
    implicitWidth: cardW + 2 * m
    implicitHeight: cardH + 2 * m

    function fileUrl(p) {
        return "file://" + p.split("/").map(encodeURIComponent).join("/");
    }

    // ---- Motion -----------------------------------------------------------
    // progress: 0 off to the right, 1 in place. offset: swipe distance.
    property real progress: 0
    property real offset: 0
    property bool leaving: false
    readonly property real travel: cardW + edge

    function enter() {
        leaving = false;
        leaveAnim.stop();
        backAnim.stop();
        offset = 0;
        enterAnim.restart();
        life.restart();
    }
    // Slide out to the right (or fade, after a drop), then unload
    function leave(fade) {
        if (leaving)
            return;
        leaving = true;
        life.stop();
        enterAnim.stop();
        leaveAnim.fade = fade === true;
        leaveAnim.restart();
    }
    NumberAnimation {
        id: enterAnim
        target: win
        property: "progress"
        to: 1
        duration: 300
        easing.type: Easing.OutCubic
    }
    SequentialAnimation {
        id: leaveAnim
        property bool fade: false
        ParallelAnimation {
            NumberAnimation { target: win; property: "progress"; to: 0; duration: 200; easing.type: Easing.InCubic }
            NumberAnimation { target: win; property: "offset"; to: leaveAnim.fade ? win.offset : win.travel; duration: 200; easing.type: Easing.InCubic }
        }
        // Deferred: this unloads the window
        ScriptAction { script: Qt.callLater(() => win.session.thumbGone()) }
    }
    NumberAnimation {
        id: backAnim
        target: win
        property: "offset"
        to: 0
        duration: 200
        easing.type: Easing.OutCubic
    }

    // 5 s for a screenshot, 8 s for the Undo toast; hovering holds it
    Timer {
        id: life
        interval: win.deleted ? 8000 : 5000
        running: !win.leaving && !hover.hovered && win.mode === ""
        onTriggered: win.leave()
    }
    Component.onCompleted: enter()
    Connections {
        target: win.session
        // A newer screenshot (or a fresh save) replaces this one
        function onSerialChanged() { win.enter(); }
        function onThumbChanged() { if (win.session.thumb !== "") win.enter(); }
    }

    // While the card is moving, only it takes input
    mask: Region { item: card }

    Item {
        id: card
        x: win.m + win.offset + (1 - win.progress) * (win.leaving && leaveAnim.fade ? 0 : win.travel)
        y: win.m
        width: win.cardW
        height: win.cardH
        opacity: win.leaving && leaveAnim.fade ? win.progress : 1
        onXChanged: glass.updatePosition()

        Glass {
            id: glass
            anchors.fill: parent
            anchors.margins: -win.m
            shadowMargin: win.m
            radius: win.deleted ? 22 : 14
            bevel: 12
        }

        HoverHandler { id: hover }

        // ---- Screenshot ---------------------------------------------------
        Item {
            id: shotView
            anchors.fill: parent
            visible: !win.deleted

            // Rounded image: 14 - 6 padding = 8
            ClippingRectangle {
                id: frame
                anchors.fill: parent
                anchors.margins: 6
                radius: 8
                color: Theme.color.fill
                Image {
                    id: preview
                    anchors.fill: parent
                    source: win.file ? win.fileUrl(win.file) + "?v=" + win.session.serial : ""
                    sourceSize: Qt.size(400, 400)
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    cache: false
                    smooth: true
                }
                // Loading: a quiet placeholder glyph
                Icon {
                    anchors.centerIn: parent
                    visible: preview.status === Image.Loading
                    name: "image"
                    size: 24
                    color: Theme.color.tertiary
                }
                // The file vanished before it could be shown
                Column {
                    anchors.centerIn: parent
                    visible: preview.status === Image.Error
                    spacing: 4
                    Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "warning-circle"; size: 20; color: Theme.color.secondary }
                    Text {
                        text: "Couldn't load preview"
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                }
                // Press feedback
                Rectangle {
                    anchors.fill: parent
                    color: "black"
                    opacity: mouse.pressed && win.mode === "" ? 0.12 : 0
                    Behavior on opacity { NumberAnimation { duration: Theme.motion.press } }
                }
            }

            // Hint while hovered: what a click does
            Rectangle {
                anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 12 }
                width: hintText.implicitWidth + 16
                height: 22
                radius: 11
                color: Qt.rgba(0.1, 0.1, 0.12, 0.78)
                opacity: hover.hovered && win.mode === "" && preview.status === Image.Ready ? 1 : 0
                Behavior on opacity { OpacityAnimator { duration: Theme.motion.hover } }
                Text {
                    id: hintText
                    anchors.centerIn: parent
                    text: "Click to mark up"
                    color: "white"
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                }
            }

            // Click to edit, drag out to drop the file, swipe right to dismiss
            MouseArea {
                id: mouse
                anchors.fill: parent
                hoverEnabled: false
                cursorShape: win.mode === "swipe" ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                acceptedButtons: Qt.LeftButton
                property point start
                onPressed: m => {
                    start = Qt.point(m.x, m.y);
                    win.mode = "";
                    backAnim.stop();
                }
                onPositionChanged: m => {
                    const dx = m.x - start.x, dy = m.y - start.y;
                    if (win.mode === "" && Math.hypot(dx, dy) > 6) {
                        if (dx > Math.abs(dy)) {
                            win.mode = "swipe";
                        } else if (preview.status === Image.Ready) {
                            win.mode = "file";
                            dragProxy.Drag.active = true; // starts the system drag (asynchronously)
                        }
                    }
                    if (win.mode === "swipe")
                        win.offset = dx > 0 ? dx : dx / 4; // gives a little the wrong way
                }
                onReleased: m => {
                    if (win.mode === "swipe") {
                        win.mode = "";
                        if (win.offset > 56)
                            win.leave();
                        else
                            backAnim.restart();
                    } else if (win.mode === "") {
                        // Deferred: opening the editor unloads this window
                        const f = win.file, ses = win.session;
                        Qt.callLater(() => ses.openEditor(f));
                    }
                }
                onCanceled: {
                    if (win.mode === "swipe") {
                        win.mode = "";
                        backAnim.restart();
                    }
                }
            }

            // Dismiss without swiping (pointer users who don't know the gesture)
            Rectangle {
                x: -6
                y: -6
                width: 22
                height: 22
                radius: 11
                color: closeMouse.pressed ? Theme.color.fillPressed : Theme.dark ? "#3A3A3F" : "#FFFFFF"
                border.width: 1
                border.color: Theme.color.separator
                opacity: hover.hovered && win.mode === "" ? 1 : 0
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
                Icon { anchors.centerIn: parent; name: "x"; bold: true; size: 11; color: Theme.color.secondary }
                MouseArea {
                    id: closeMouse
                    anchors.fill: parent
                    anchors.margins: -6
                    cursorShape: Qt.PointingHandCursor
                    onClicked: win.leave()
                }
            }
        }

        // ---- Undo toast after Delete -----------------------------------------
        Row {
            id: toastRow
            visible: win.deleted
            anchors.centerIn: parent
            spacing: 12
            Icon { anchors.verticalCenter: parent.verticalCenter; name: "trash"; size: 16; color: Theme.color.secondary }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Screenshot deleted"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
            }
            Squircle {
                anchors.verticalCenter: parent.verticalCenter
                width: undoText.implicitWidth + 20
                height: 28
                radius: Theme.radius.control
                color: undoMouse.pressed ? Theme.color.fillPressed : undoMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                scale: undoMouse.pressed ? 0.97 : 1
                Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
                Text {
                    id: undoText
                    anchors.centerIn: parent
                    text: "Undo"
                    color: Theme.accentInk
                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                }
                MouseArea {
                    id: undoMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        const ses = win.session;
                        Qt.callLater(() => ses.undoDelete());
                    }
                }
            }
        }
    }

    // "" idle | swipe | file (a system drag is running)
    property string mode: ""

    // The system drag: the file as a URI list, with the thumbnail as its image
    Item {
        id: dragProxy
        width: 1
        height: 1
        Drag.dragType: Drag.Automatic
        Drag.supportedActions: Qt.CopyAction
        Drag.proposedAction: Qt.CopyAction
        Drag.mimeData: ({ "text/uri-list": win.fileUrl(win.file) + "\r\n" })
        Drag.hotSpot: Qt.point(40, 30)
        Drag.onDragFinished: action => {
            win.mode = "";
            if (action !== Qt.IgnoreAction)
                win.leave(true); // dropped somewhere: done, like macOS
            else
                life.restart();
        }
        Drag.onActiveChanged: if (!Drag.active && win.mode === "file") win.mode = ""
    }
    // A small picture to drag (the full screenshot would be huge)
    Connections {
        target: preview
        function onStatusChanged() {
            if (preview.status === Image.Ready)
                Qt.callLater(() => frame.grabToImage(r => dragProxy.Drag.imageSource = r.url, Qt.size(Math.round(win.thumbW * 0.6), Math.round(win.thumbH * 0.6))));
        }
    }
}
