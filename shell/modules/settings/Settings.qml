import QtQuick
import Quickshell
import qs.common

// iRice Settings: a regular app window. Translucent sidebar of sections on the
// left (the compositor blurs what's behind it), the selected pane on the right.
Scope {
    LazyLoader {
        active: Panels.settingsOpen

        FloatingWindow {
            id: win
            title: "iRice Settings"
            implicitWidth: 860
            implicitHeight: 620
            minimumSize: Qt.size(720, 480)
            color: "transparent"
            onClosed: Panels.settingsOpen = false

            readonly property var sections: [
                { id: "appearance", name: "Appearance", icon: "circle-half", color: "#5E5CE6" },
                { id: "wallpaper", name: "Wallpaper", icon: "image", color: "#32ADE6" },
                { id: "menubar", name: "Menu Bar", icon: "list", color: "#8E8E93" },
                { id: "desktop", name: "Desktop & Weather", icon: "squares-four", color: "#FF9F0A" },
                { id: "controlcenter", name: "Control Center", icon: "sliders-horizontal", color: "#64D2FF" },
                { id: "dock", name: "Dock", icon: "dots-three-outline", color: "#0A84FF" },
                { id: "windowrules", name: "Window Rules", icon: "app-window", color: "#FF375F" },
                { id: "notifications", name: "Notifications", icon: "bell", color: "#FF453A" },
                { id: "lock", name: "Lock Screen", icon: "lock", color: "#636366" },
                { id: "sharing", name: "Sharing", icon: "share-network", color: "#0A84FF" },
                { id: "focus", name: "Focus", icon: "moon", color: "#5E5CE6" },
                { id: "calendar", name: "Calendar", icon: "calendar-blank", color: "#FF453A" },
                { id: "ai", name: "AI", icon: "sparkle", color: "#BF5AF2" },
                { id: "sidebar", name: "Sidebar", icon: "sidebar-simple", color: "#FF9F0A" },
                { id: "trackpad", name: "Trackpad", icon: "hand-swipe-right", color: "#636366" },
                { id: "shortcuts", name: "Keyboard Shortcuts", icon: "keyboard", color: "#636366" },
                { id: "about", name: "About", icon: "info", color: "#30D158" }
            ]

            Rectangle {
                id: sidebar
                width: 230
                height: parent.height
                color: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.78) : Qt.rgba(0.94, 0.94, 0.96, 0.8)

                Column {
                    anchors { fill: parent; margins: 10; topMargin: 16 }
                    spacing: 2

                    Repeater {
                        model: win.sections
                        Rectangle {
                            id: item
                            required property var modelData
                            readonly property bool current: Panels.settingsPage === modelData.id
                            width: parent.width
                            height: 32
                            radius: 8
                            color: current ? Theme.accent : mouse.containsMouse ? Theme.color.fill : "transparent"

                            Rectangle {
                                id: tile
                                x: 6
                                anchors.verticalCenter: parent.verticalCenter
                                width: 22
                                height: 22
                                radius: 6
                                color: item.modelData.color
                                Icon { anchors.centerIn: parent; name: item.modelData.icon; fill: true; size: 13; color: "white" }
                            }
                            Text {
                                anchors { left: tile.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                                text: item.modelData.name
                                color: item.current ? Theme.color.onAccent : Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: item.current ? Font.DemiBold : Font.Normal }
                            }
                            MouseArea {
                                id: mouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: Panels.settingsPage = item.modelData.id
                            }
                        }
                    }
                }
            }

            Rectangle {
                anchors { left: sidebar.right; right: parent.right; top: parent.top; bottom: parent.bottom }
                color: Theme.dark ? "#1E1E21" : "#F5F5F7"

                Flickable {
                    id: scroller
                    anchors.fill: parent
                    contentHeight: page.implicitHeight + 64
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: page
                        x: 28
                        y: 24
                        width: scroller.width - 56
                        spacing: 22

                        Text {
                            text: win.sections.find(s => s.id === Panels.settingsPage)?.name ?? ""
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.Bold }
                        }

                        Loader {
                            width: parent.width
                            source: Qt.resolvedUrl("pages/" + Panels.settingsPage + ".qml")
                            onLoaded: scroller.contentY = 0
                        }
                    }
                }
            }
        }
    }
}
