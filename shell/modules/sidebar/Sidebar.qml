import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass

// Left sidebar: a floating glass panel with AI chat, translator and the
// anime image browser. Slides in from the left; outside click or Esc closes.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "sidebar"
        screen: modelData
        linger: 60000 // freed a minute after closing (the chat lives in the Ai service)

        GlassWindow {
            id: win
            readonly property var modelData: slot.modelData
            screen: modelData
            name: "sidebar"

            readonly property bool shown: Panels.isOpen("sidebar", screen)
            readonly property int m: 24
            readonly property var tabs: Config.options.sidebar.booru ? ["AI", "Translate", "Anime"] : ["AI", "Translate"]
            property alias tab: tabState.value
            QtObject {
                id: tabState
                property int value: Panels.sidebarTab
                onValueChanged: Panels.sidebarTab = value
            }

            // Starts at 0 so the window animates in when it is created
            property real progress: 0
            Component.onCompleted: progress = Qt.binding(() => shown ? 1 : 0)
            Behavior on progress {
                NumberAnimation { duration: Theme.motion.panel; easing.type: Easing.BezierSpline; easing.bezierCurve: win.shown ? Theme.motion.emphasized : Theme.motion.exit }
            }

            visible: progress > 0 && closeGuard.open

            CloseGuard { id: closeGuard; shown: win.shown }
            WlrLayershell.layer: WlrLayer.Top
            WlrLayershell.keyboardFocus: shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            // Full-screen transparent layer: the panel on the left, and a click
            // catcher everywhere below the menu bar that closes it.
            anchors { top: true; bottom: true; left: true; right: true }
            readonly property int dockClearance: Config.options.dock.iconSize + 14 + 6 + 8

            mask: Region {
                Region { item: catcher }
                Region { item: body }
            }
            MouseArea {
                id: catcher
                y: 30
                width: parent.width
                height: parent.height - 30
                enabled: win.shown
                onClicked: Panels.close()
            }

            Item {
                id: body
                width: 420 + 2 * win.m
                height: parent.height - (30 + 6 - win.m) - (win.dockClearance - win.m)
                x: 8 - win.m + (1 - win.progress) * -40
                y: 30 + 6 - win.m
                onYChanged: glass.updatePosition()
                opacity: win.progress
                focus: win.shown
                Keys.onEscapePressed: Panels.close()
                onXChanged: glass.updatePosition()

                Glass {
                    id: glass
                    anchors.fill: parent
                    radius: 26
                    bevel: 20
                    shadowMargin: win.m
                }

                Item {
                    anchors { fill: parent; margins: win.m + 12 }

                    Segmented {
                        id: tabBar
                        width: parent.width
                        options: win.tabs
                        current: win.tab
                        onSelected: i => win.tab = i
                    }

                    Item {
                        anchors { top: tabBar.bottom; topMargin: 12; left: parent.left; right: parent.right; bottom: parent.bottom }

                        AiChat {
                            anchors.fill: parent
                            visible: win.tab === 0
                            active: win.shown && visible
                        }
                        Translator {
                            anchors.fill: parent
                            visible: win.tab === 1
                            active: win.shown && visible
                        }
                        Loader {
                            anchors.fill: parent
                            active: Config.options.sidebar.booru && win.tab === 2
                            visible: active
                            sourceComponent: Booru {}
                        }
                    }
                }
            }
        }
    }
}
