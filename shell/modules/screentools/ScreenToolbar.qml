import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// ⇧⌘5-style toolbar: capture screen / selection, copy text, translate,
// record screen / selection. Esc closes.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "screenTools"
        screen: modelData

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "screenTools"

            readonly property bool shown: Panels.isOpen("screenTools", screen)
            readonly property int m: 24
            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            readonly property var tools: [
                { icon: "monitor", label: "Capture Screen", run: () => ScreenTools.screenshotScreen() },
                { icon: "selection", label: "Capture Selection", run: () => ScreenTools.select("screenshot") },
                { sep: true },
                { icon: "text-t", label: "Copy Text", run: () => ScreenTools.select("ocr") },
                { icon: "translate", label: "Translate", run: () => ScreenTools.select("translate") },
                { icon: "sparkle", label: "Ask AI", run: () => ScreenTools.select("ask") },
                { sep: true },
                { icon: "record", label: "Record Screen", run: () => ScreenTools.recordScreen() },
                { icon: "selection-plus", label: "Record Selection", run: () => ScreenTools.select("record") }
            ]

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            // Input only while shown: during the closing fade (or if it ever
            // stalls) the window lets clicks through instead of eating them
            mask: Region { width: win.shown ? win.width : 0; height: win.shown ? win.height : 0 }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            anchors.bottom: true
            margins.bottom: Config.options.dock.iconSize + 14 + 6 + 14 - m
            implicitWidth: row.implicitWidth + 24 + 2 * m
            implicitHeight: 64 + 2 * m

            Item {
                anchors.fill: parent
                opacity: win.progress
                y: (1 - win.progress) * 12
                onYChanged: glass.updatePosition()
                focus: win.shown
                Keys.onEscapePressed: Panels.close()

                Glass {
                    id: glass
                    anchors.fill: parent
                    radius: 22
                    bevel: 18
                    shadowMargin: win.m
                }

                Row {
                    id: row
                    anchors.centerIn: parent
                    spacing: 4

                    Repeater {
                        model: win.tools
                        Item {
                            id: tool
                            required property var modelData
                            width: modelData.sep ? 17 : 104
                            height: 56

                            Rectangle {
                                visible: !!tool.modelData.sep
                                anchors.centerIn: parent
                                width: 1
                                height: 36
                                color: Theme.color.separator
                            }
                            Rectangle {
                                visible: !tool.modelData.sep
                                anchors.fill: parent
                                radius: 12
                                color: toolMouse.pressed ? Theme.color.fillPressed : toolMouse.containsMouse ? Theme.color.fill : "transparent"
                                Column {
                                    anchors.centerIn: parent
                                    spacing: 4
                                    Icon {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        name: tool.modelData.icon ?? ""
                                        size: 22
                                        color: (tool.modelData.icon === "record" && ScreenTools.recording) ? Theme.color.danger : Theme.color.label
                                    }
                                    Text {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        text: tool.modelData.label ?? ""
                                        color: Theme.color.secondary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                                    }
                                }
                                MouseArea {
                                    id: toolMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onClicked: tool.modelData.run()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
