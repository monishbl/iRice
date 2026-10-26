import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services
import "elements.js" as PT

// Cheatsheet: keyboard shortcuts (read live from Hyprland) and a periodic table.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "cheatsheet"
        screen: modelData

        Dialog {
            id: dialog
            readonly property var modelData: slot.modelData
            screen: modelData
            dialogName: "cheatsheet"
            contentWidth: Math.min(1100, (screen?.width ?? 1920) - 160)
            contentHeight: Math.min(660, (screen?.height ?? 1080) - 200)
            property int tab: 0
            onShownChanged: if (shown) Keybinds.refresh()
            Component.onCompleted: Keybinds.refresh()

            // Category tints (chart palette, quiet)
            readonly property var tints: Theme.dark
                ? ["#F5886C", "#E7963D", "#93A3FC", "#6EC272", "#25C2C2", "#BF93F1", "#F3829F", "#E087CE", "#8FB8FF", "#C6A26A"]
                : ["#D05F43", "#BE7100", "#6D7CDF", "#409D48", "#1D9999", "#9B6BCE", "#CE597A", "#BB5FAA", "#4F7FD8", "#8F6F3A"]

            onTabPressed: tab = 1 - tab // Tab switches tabs

            Segmented {
                id: tabs
                anchors.horizontalCenter: parent.horizontalCenter
                width: 300
                options: ["Shortcuts", "Periodic Table"]
                current: dialog.tab
                onSelected: i => dialog.tab = i
            }

            // Shortcuts, flowed into columns
            Flickable {
                visible: dialog.tab === 0
                anchors { top: tabs.bottom; topMargin: 18; left: parent.left; right: parent.right; bottom: parent.bottom }
                contentHeight: flow.implicitHeight
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                Flow {
                    id: flow
                    width: parent.width
                    spacing: 24

                    Repeater {
                        model: Keybinds.groups
                        Column {
                            id: group
                            required property var modelData
                            width: (flow.width - 2 * 24) / 3
                            spacing: 6
                            Text {
                                text: group.modelData.name
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                            }
                            Repeater {
                                model: group.modelData.binds
                                Item {
                                    id: bind
                                    required property var modelData
                                    width: group.width
                                    height: 26
                                    Text {
                                        anchors { left: parent.left; right: combo.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                                        text: bind.modelData.description
                                        elide: Text.ElideRight
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                                    }
                                    KeyCombo {
                                        id: combo
                                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                        keys: bind.modelData.keys
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Periodic table
            Item {
                id: table
                visible: dialog.tab === 1
                anchors { top: tabs.bottom; topMargin: 18; left: parent.left; right: parent.right; bottom: parent.bottom }
                readonly property real cell: Math.min(width / 18, height / 9.4)
                property var hovered: null

                Item {
                    width: table.cell * 18
                    height: table.cell * 9.4
                    anchors.horizontalCenter: parent.horizontalCenter

                    Repeater {
                        model: PT.elements
                        Rectangle {
                            id: el
                            required property var modelData
                            readonly property int row: modelData[5]
                            readonly property color tint: dialog.tints[PT.categories[modelData[4]][1]]
                            x: (modelData[6] - 1) * table.cell + 1
                            y: (row <= 7 ? row - 1 : row - 2.6) * table.cell + 1
                            width: table.cell - 2
                            height: table.cell - 2
                            radius: 6
                            color: Qt.alpha(tint, table.hovered === modelData ? 0.45 : 0.18)
                            border.width: table.hovered === modelData ? 1 : 0
                            border.color: tint

                            Text {
                                x: 4
                                y: 2
                                text: el.modelData[0]
                                color: Theme.color.secondary
                                font { family: Theme.font.family; pixelSize: Math.max(8, table.cell * 0.17) }
                            }
                            Text {
                                anchors.centerIn: parent
                                anchors.verticalCenterOffset: 2
                                text: el.modelData[1]
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: table.cell * 0.36; weight: Font.DemiBold }
                            }
                            MouseArea {
                                anchors.fill: parent
                                hoverEnabled: true
                                onEntered: table.hovered = el.modelData
                                onExited: if (table.hovered === el.modelData) table.hovered = null
                            }
                        }
                    }

                    // Detail for the hovered element, in the empty space over groups 3–12
                    Column {
                        x: 2.3 * table.cell
                        y: 0.1 * table.cell
                        spacing: 2
                        visible: table.hovered !== null
                        Text {
                            text: table.hovered ? table.hovered[2] : ""
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.large; weight: Font.Bold }
                        }
                        Text {
                            text: table.hovered ? "Atomic number " + table.hovered[0] + " · " + table.hovered[3] + " u · " + PT.categories[table.hovered[4]][0] : ""
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.body }
                        }
                    }
                }
            }
        }
    }
}
