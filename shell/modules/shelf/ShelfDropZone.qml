import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// The island's drop zone (Shelf.zone): a drag of files that asked for the
// shelf (a shake, an edge) gets two big targets under the menu bar, in the
// island's black: keep them on the shelf (it shrinks to the shelf's card),
// or send them to a phone (the share sheet: Quick Share or LocalSend). It
// goes if the drag ends elsewhere, a moment after the drag leaves it, or
// with a click on it or its close button (Shelf.offer).
// Material: a surfaceContainer card (corner 28), the targets tonal, the one
// under the drag in primaryContainer outlined in primary.
LazyLoader {
    id: loader
    property bool leaving: false
    active: Shelf.zone || leaving
    Connections {
        target: Shelf
        function onZoneChanged() {
            if (!Shelf.zone) {
                loader.leaving = true;
                leaveTimer.restart();
            }
        }
    }
    Timer { id: leaveTimer; interval: 200; onTriggered: loader.leaving = false }

    PanelWindow {
        id: win
        screen: Shelf.zoneScreen ?? Quickshell.screens[0]
        color: "transparent"
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "irice:shelf-zone"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        anchors.top: true
        margins.top: 4
        implicitWidth: 560
        implicitHeight: zone.height + 24
        mask: Region { item: zone }

        // In with a spring from the island's size, out a little faster
        property real shown: 0
        Component.onCompleted: shown = Qt.binding(() => Shelf.zone ? 1 : 0)
        Behavior on shown {
            NumberAnimation {
                duration: Shelf.zone ? Math.round(320 * CoolDown.motionScale) : 180
                easing.type: Shelf.zone && CoolDown.motionScale >= 1 ? Easing.OutBack : Easing.OutCubic
                easing.overshoot: 1.2
            }
        }

        Rectangle {
            id: zone
            anchors.horizontalCenter: parent.horizontalCenter
            width: 520
            height: column.implicitHeight + 32
            radius: Theme.material ? 28 : 32
            color: Theme.material ? Theme.m3.surfaceContainer : "#000000"
            border.width: Theme.material ? 0 : 1
            border.color: Qt.rgba(1, 1, 1, 0.12)
            opacity: win.shown
            scale: 0.72 + 0.28 * win.shown
            transformOrigin: Item.Top

            // The drag over it (anywhere: the edge, a target) keeps it; a
            // click on it (once the drag is over) puts it away
            readonly property bool hovering: zoneArea.containsDrag || keepTarget.over || sendTarget.over
            onHoveringChanged: Shelf.zoneHover(hovering)
            DropArea {
                id: zoneArea
                anchors.fill: parent
                // (only to know the drag is here: a copy at most, and a drop
                // between the targets does nothing)
                onEntered: d => d.accept(Qt.CopyAction)
                onDropped: d => d.accepted = false
            }
            MouseArea {
                anchors.fill: parent
                onClicked: Shelf.cancelZone()
            }

            Column {
                id: column
                x: 16
                y: 16
                width: parent.width - 32
                spacing: 14

                Item {
                    width: parent.width
                    height: 22
                    Icon {
                        id: headIcon
                        anchors.verticalCenter: parent.verticalCenter
                        x: 4
                        name: "tray-arrow-down"
                        fill: true
                        size: 18
                        color: Theme.material ? Theme.m3.primary : "#F5F5F7"
                    }
                    Text {
                        anchors { left: headIcon.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                        text: "Drop to keep or send"
                        color: Theme.material ? Theme.m3.onSurface : "#F5F5F7"
                        font { family: Theme.font.family; pixelSize: 15; weight: Font.DemiBold }
                    }
                    Text {
                        anchors { right: closeButton.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
                        visible: Shelf.count > 0
                        text: Shelf.count + " on the shelf"
                        color: Theme.material ? Theme.m3.onSurfaceVariant : Qt.rgba(1, 1, 1, 0.6)
                        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                    }
                    Rectangle {
                        id: closeButton
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        width: 28
                        height: 28
                        radius: 14
                        // (Material: a standard icon button, its state layer)
                        color: Theme.material ? Qt.alpha(Theme.m3.onSurfaceVariant, closeMouse.containsMouse ? 0.08 : 0)
                            : closeMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.2) : Qt.rgba(1, 1, 1, 0.1)
                        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                        Accessible.role: Accessible.Button
                        Accessible.name: "Close"
                        Icon {
                            anchors.centerIn: parent
                            name: "x"
                            bold: true
                            size: 14
                            color: Theme.material ? Theme.m3.onSurfaceVariant : "#F5F5F7"
                        }
                        MouseArea {
                            id: closeMouse
                            anchors.fill: parent
                            anchors.margins: -8 // (a bigger target than it looks)
                            hoverEnabled: true
                            onClicked: Shelf.cancelZone()
                        }
                    }
                }

                Row {
                    spacing: 12
                    Target {
                        id: keepTarget
                        icon: "tray"
                        title: "Keep on the shelf"
                        detail: "Park them, take them out later"
                        onDroppedHere: (urls, text) => Shelf.keep(urls, text)
                    }
                    Target {
                        id: sendTarget
                        icon: "share-network"
                        title: "Send to a phone"
                        detail: "Quick Share or LocalSend"
                        onDroppedHere: (urls, text) => Shelf.sendOn(urls)
                    }
                }
            }
        }
    }

    component Target: Rectangle {
        id: target
        property string icon
        property string title
        property string detail
        signal droppedHere(var urls, string text)
        width: 236
        height: 116
        readonly property bool over: dropArea.containsDrag
        // (Material: tonal, the one under the drag in primaryContainer with a
        // primary outline, its corners rounder)
        radius: Theme.material ? (over ? 28 : 20) : 20
        Behavior on radius {
            enabled: Theme.material
            MaterialSpring { speed: "fast"; epsilon: 0.25 }
        }
        color: Theme.material ? (over ? Theme.m3.primaryContainer : Theme.m3.surfaceContainerHighest) : over ? Qt.alpha(Theme.accent, 0.38) : Qt.rgba(1, 1, 1, 0.08)
        border.width: Theme.material ? (over ? 2 : 0) : 1
        border.color: Theme.material ? Theme.m3.primary : over ? Theme.accent : Qt.rgba(1, 1, 1, 0.12)
        scale: over ? 1.03 : 1
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        Behavior on scale { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
        Column {
            anchors.centerIn: parent
            spacing: 6
            Icon {
                anchors.horizontalCenter: parent.horizontalCenter
                name: target.icon
                fill: true
                size: 28
                color: Theme.material ? (target.over ? Theme.m3.onPrimaryContainer : Theme.m3.primary) : "#F5F5F7"
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: target.title
                color: Theme.material ? (target.over ? Theme.m3.onPrimaryContainer : Theme.m3.onSurface) : "#F5F5F7"
                font { family: Theme.font.family; pixelSize: 14; weight: Font.DemiBold }
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: target.detail
                color: Theme.material ? (target.over ? Theme.m3.onPrimaryContainer : Theme.m3.onSurfaceVariant) : Qt.rgba(1, 1, 1, 0.6)
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
        DropArea {
            id: dropArea
            anchors.fill: parent
            onEntered: d => d.accept(Qt.CopyAction)
            onDropped: d => {
                target.droppedHere(d.urls, d.text ?? "");
                d.accept(Qt.CopyAction);
            }
        }
    }
}
