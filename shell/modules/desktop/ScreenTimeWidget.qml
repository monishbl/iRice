import QtQuick
import qs.common
import qs.services

// Screen Time: how long you've used the computer today and on which apps.
// Medium adds the week as bars.
Item {
    id: root

    property string size: "s"
    property bool preview: false
    readonly property var apps: ScreenTime.todayApps.slice(0, size === "s" ? 2 : 3)
    readonly property int most: ScreenTime.todayApps[0]?.seconds ?? 1

    WidgetHeader { id: header; icon: "hourglass-medium"; title: "Screen Time"; tint: Theme.accents.purple }

    Column {
        id: left
        anchors { left: parent.left; top: header.bottom; topMargin: 10 }
        width: root.size === "s" ? parent.width : parent.width * 0.48
        spacing: 8
        Text {
            text: ScreenTime.todayTotal > 0 ? ScreenTime.format(ScreenTime.todayTotal) : "—"
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; features: { "tnum": 1 } }
        }
        // The week, today last
        Row {
            visible: root.size !== "s"
            spacing: 6
            readonly property int maxSecs: Math.max(1, ...ScreenTime.week.map(d => d.seconds))
            Repeater {
                model: ScreenTime.week
                Column {
                    id: bar
                    required property var modelData
                    required property int index
                    spacing: 3
                    Item {
                        width: 12
                        height: 44
                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: Math.max(2, parent.height * bar.modelData.seconds / bar.parent.maxSecs)
                            radius: 3
                            color: bar.index === 6 ? Theme.accents.purple : Theme.color.fill
                        }
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: bar.modelData.label
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                    }
                }
            }
        }
    }

    // Most used apps today
    Column {
        anchors { left: root.size === "s" ? parent.left : left.right; leftMargin: root.size === "s" ? 0 : 12; right: parent.right; bottom: parent.bottom }
        spacing: 6
        Text {
            visible: root.apps.length === 0
            text: "Nothing yet today"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Repeater {
            model: root.apps
            Row {
                id: appRow
                required property var modelData
                width: parent.width
                spacing: 6
                readonly property var entry: Apps.entryFor(modelData.id)
                AppIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    size: 18
                    source: Apps.iconFor(appRow.entry ?? appRow.modelData.id)
                }
                Column {
                    width: parent.width - 24
                    spacing: 2
                    Item {
                        width: parent.width
                        height: nameText.implicitHeight
                        Text {
                            id: nameText
                            width: parent.width - timeText.implicitWidth - 6
                            text: appRow.entry?.name ?? appRow.modelData.id
                            elide: Text.ElideRight
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
                        }
                        Text {
                            id: timeText
                            anchors.right: parent.right
                            text: ScreenTime.format(appRow.modelData.seconds)
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                        }
                    }
                    Rectangle {
                        width: parent.width * appRow.modelData.seconds / Math.max(1, root.most)
                        height: 3
                        radius: 1.5
                        color: Theme.accents.purple
                        opacity: 0.8
                    }
                }
            }
        }
    }
}
