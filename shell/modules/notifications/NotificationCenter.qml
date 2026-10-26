import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.modules.controlcenter
import qs.services

// Opened from the clock: notification history above, widgets below.
Variants {
    model: Quickshell.screens

    PanelLoader {
        id: slot
        required property var modelData
        panel: "notificationCenter"
        screen: modelData

        Popover {
            id: nc
            readonly property var modelData: slot.modelData
            screen: modelData
            popoverName: "notificationCenter"
            contentWidth: 340
            targetX: (screen?.width ?? 1920) - implicitWidth
            radius: Theme.radius.module + padding // concentric with the platters inside
            property alias widgetTab: ncTabState.value
            QtObject {
                id: ncTabState
                property int value: Panels.ncTab
                onValueChanged: Panels.ncTab = value
            }

            Column {
                width: nc.contentWidth
                spacing: 10

                Item {
                    width: parent.width
                    height: 28
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 4
                        text: "Notifications"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Rectangle {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        visible: Notifications.list.length > 0
                        width: clearLabel.implicitWidth + 20
                        height: 26
                        radius: 13
                        color: clearMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                        Text {
                            id: clearLabel
                            anchors.centerIn: parent
                            text: "Clear all"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                        }
                        MouseArea {
                            id: clearMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: Notifications.clearAll()
                        }
                    }
                }

                // Empty state
                Platter {
                    visible: Notifications.list.length === 0
                    width: parent.width
                    height: 84
                    Column {
                        anchors.centerIn: parent
                        spacing: 6
                        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "bell-simple"; size: 22; color: Theme.color.tertiary }
                        Text {
                            text: Focus.on ? Focus.mode.name + " is on" : "No notifications"
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                }

                Flickable {
                    visible: Notifications.list.length > 0
                    width: parent.width
                    height: Math.min(list.implicitHeight, 420)
                    contentHeight: list.implicitHeight
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    // Grouped by app, newest group first; groups of two or more
                    // stack and expand on click, like macOS
                    Column {
                        id: list
                        width: parent.width
                        spacing: 8
                        property var expandedApps: ({})
                        readonly property var groups: {
                            const out = [];
                            for (const n of Notifications.list.slice().reverse()) {
                                const app = n.appName || "Notification";
                                let g = out.find(x => x.app === app);
                                if (!g)
                                    out.push(g = { app: app, items: [] });
                                g.items.push(n);
                            }
                            return out;
                        }
                        Repeater {
                            model: list.groups
                            Column {
                                id: group
                                required property var modelData
                                readonly property bool stacked: modelData.items.length > 1
                                readonly property bool open: stacked && !!list.expandedApps[modelData.app]
                                width: list.width
                                spacing: 6

                                // Header for an expanded group
                                Item {
                                    visible: group.open
                                    width: parent.width
                                    height: 24
                                    Text {
                                        x: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: group.modelData.app
                                        color: Theme.color.label
                                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                                    }
                                    Row {
                                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                        spacing: 6
                                        Rectangle {
                                            width: lessLabel.implicitWidth + 18
                                            height: 22
                                            radius: 11
                                            color: lessMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                                            Text {
                                                id: lessLabel
                                                anchors.centerIn: parent
                                                text: "Show less"
                                                color: Theme.color.label
                                                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                                            }
                                            MouseArea {
                                                id: lessMouse
                                                anchors.fill: parent
                                                hoverEnabled: true
                                                onClicked: {
                                                    const e = Object.assign({}, list.expandedApps);
                                                    delete e[group.modelData.app];
                                                    list.expandedApps = e;
                                                }
                                            }
                                        }
                                        IconButton {
                                            width: 22
                                            height: 22
                                            icon: "x"
                                            iconSize: 11
                                            onClicked: { for (const n of group.modelData.items.slice()) n.dismiss(); }
                                        }
                                    }
                                }

                                Repeater {
                                    model: group.open ? group.modelData.items : group.modelData.items.slice(0, 1)
                                    Item {
                                        id: entry
                                        required property var modelData
                                        required property int index
                                        width: list.width
                                        // Collapsed stacks show edges of the next two cards below
                                        height: card.implicitHeight + (group.stacked && !group.open ? 8 * Math.min(group.modelData.items.length - 1, 2) : 0)

                                        Repeater {
                                            model: group.stacked && !group.open ? Math.min(group.modelData.items.length - 1, 2) : 0
                                            Platter {
                                                required property int index
                                                x: (index + 1) * 8
                                                // Just the lower edge, below the front card
                                                y: card.implicitHeight - 4 + index * 8
                                                z: -1 - index
                                                width: list.width - 2 * (index + 1) * 8
                                                height: 12
                                                radius: 6
                                                opacity: 0.75 - 0.2 * index
                                            }
                                        }
                                        Platter {
                                            width: list.width
                                            height: card.implicitHeight
                                            radius: 18
                                            NotificationCard {
                                                id: card
                                                width: parent.width
                                                notification: entry.modelData
                                                onCloseRequested: entry.modelData.dismiss()
                                            }
                                            // A collapsed stack expands instead of opening the notification
                                            MouseArea {
                                                anchors.fill: parent
                                                visible: group.stacked && !group.open
                                                onClicked: {
                                                    const e = Object.assign({}, list.expandedApps);
                                                    e[group.modelData.app] = true;
                                                    list.expandedApps = e;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Segmented {
                    width: parent.width
                    options: ["Calendar", "To-do", "Focus"]
                    current: nc.widgetTab
                    onSelected: i => nc.widgetTab = i
                }
                CalendarWidget {
                    width: parent.width
                    visible: nc.widgetTab === 0
                }
                TodoWidget {
                    width: parent.width
                    visible: nc.widgetTab === 1
                }
                FocusWidget {
                    width: parent.width
                    visible: nc.widgetTab === 2
                }
            }
        }
    }
}
