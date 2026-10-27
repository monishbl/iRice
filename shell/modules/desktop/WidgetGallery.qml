import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Edit Widgets: a glass sheet along the bottom of the screen, like macOS.
// Pick a widget on the left, then click one of its sizes to add it to the
// desktop (it lands in the first free cell). Done or Escape ends editing.
Variants {
    model: Quickshell.screens

    Loader {
        id: slot
        required property var modelData
        active: Panels.widgetEditing && Panels.screen === modelData
        sourceComponent: GlassWindow {
            id: win
            screen: slot.modelData
            name: "widgetgallery"
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
            exclusionMode: ExclusionMode.Ignore
            anchors.bottom: true
            margins.bottom: 96 // above the dock
            implicitWidth: Math.min(880, (screen?.width ?? 1280) - 48) + 2 * m
            implicitHeight: 316 + 2 * m
            readonly property int m: 24

            WidgetDefs { id: widgetDefs }
            property int picked: 0
            readonly property var kind: widgetDefs.kinds[picked]
            readonly property real previewScale: 0.56

            property real progress: 0
            Component.onCompleted: {
                progress = 1;
                body.forceActiveFocus();
            }
            Behavior on progress { SpringAnimation { spring: 4; damping: 0.32; epsilon: 0.004 } }

            Item {
                id: body
                anchors.fill: parent
                anchors.margins: win.m
                opacity: Math.min(1, win.progress)
                transform: Translate { y: (1 - win.progress) * 40 }
                Keys.onEscapePressed: Panels.widgetEditing = false
                Keys.onUpPressed: win.picked = Math.max(0, win.picked - 1)
                Keys.onDownPressed: win.picked = Math.min(widgetDefs.kinds.length - 1, win.picked + 1)

                Glass {
                    anchors.fill: parent
                    anchors.margins: -win.m
                    shadowMargin: win.m
                    radius: 28
                }

                // Kinds
                ListView {
                    id: kinds
                    x: 12
                    y: 12
                    width: 200
                    height: parent.height - 24
                    clip: true
                    spacing: 2
                    model: widgetDefs.kinds
                    currentIndex: win.picked
                    boundsBehavior: Flickable.StopAtBounds
                    delegate: Rectangle {
                        id: row
                        required property var modelData
                        required property int index
                        width: kinds.width
                        height: 36
                        radius: 10
                        color: index === win.picked ? Theme.color.fill : rowMouse.containsMouse ? Theme.color.fillHover : "transparent"
                        Icon {
                            x: 10
                            anchors.verticalCenter: parent.verticalCenter
                            name: row.modelData.icon
                            size: 18
                            color: row.index === win.picked ? Theme.color.label : Theme.color.secondary
                        }
                        Text {
                            x: 38
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.modelData.label
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: row.index === win.picked ? Font.DemiBold : Font.Normal }
                        }
                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: win.picked = row.index
                        }
                    }
                }

                // Detail: name, what it shows, and its sizes
                Item {
                    anchors { left: kinds.right; leftMargin: 24; right: parent.right; rightMargin: 20; top: parent.top; topMargin: 16; bottom: parent.bottom; bottomMargin: 16 }

                    Column {
                        id: title
                        width: parent.width - done.width - 16
                        spacing: 4
                        Text {
                            text: win.kind.label
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            text: win.kind.desc + " Click a size to add it."
                            wrapMode: Text.Wrap
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                    Rectangle {
                        id: done
                        anchors.right: parent.right
                        width: doneLabel.implicitWidth + 32
                        height: 32
                        radius: 16
                        color: doneMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent
                        Text { id: doneLabel; anchors.centerIn: parent; text: "Done"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold } }
                        MouseArea { id: doneMouse; anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Panels.widgetEditing = false }
                    }

                    // Size previews: the real widget, scaled down
                    Row {
                        anchors { left: parent.left; bottom: parent.bottom }
                        spacing: 20
                        Repeater {
                            model: win.kind.sizes
                            Column {
                                id: sizeCol
                                required property string modelData
                                readonly property size px: widgetDefs.pixelSize(modelData)
                                spacing: 8
                                Item {
                                    width: sizeCol.px.width * win.previewScale
                                    height: sizeCol.px.height * win.previewScale
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    scale: addMouse.pressed ? 0.95 : addMouse.containsMouse ? 1.04 : 1
                                    Behavior on scale { SpringAnimation { spring: 5; damping: 0.35; epsilon: 0.002 } }
                                    Rectangle {
                                        width: sizeCol.px.width
                                        height: sizeCol.px.height
                                        scale: win.previewScale
                                        transformOrigin: Item.TopLeft
                                        radius: Theme.radius.tile
                                        color: Theme.color.platter
                                        border.width: addMouse.containsMouse ? 3 : 1
                                        border.color: addMouse.containsMouse ? Theme.accent : Qt.alpha("white", Theme.dark ? 0.08 : 0.4)
                                        clip: true
                                        Loader {
                                            anchors.fill: parent
                                            anchors.margins: win.kind.bleed ? 0 : 16
                                            source: Qt.resolvedUrl(win.kind.file)
                                            onLoaded: {
                                                item.size = sizeCol.modelData;
                                                if (item.preview !== undefined)
                                                    item.preview = true;
                                                if (item.radius !== undefined)
                                                    item.radius = 22;
                                            }
                                        }
                                    }
                                    // Add badge
                                    Rectangle {
                                        x: -8
                                        y: -8
                                        width: 22
                                        height: 22
                                        radius: 11
                                        color: Theme.accents.green
                                        visible: addMouse.containsMouse
                                        Icon { anchors.centerIn: parent; name: "plus"; bold: true; size: 12; color: "white" }
                                    }
                                    MouseArea {
                                        id: addMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: Panels.addWidget(win.kind.id, sizeCol.modelData)
                                    }
                                }
                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: widgetDefs.sizeNames[sizeCol.modelData]
                                    color: Theme.color.secondary
                                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
