import QtQuick
import Quickshell
import qs.common
import qs.services

// The open island's Recent list: the last few alerts (Island.history, the
// ones Notification Center keeps under iRice), newest first. One compact line
// each: the alert's icon, what and its detail, and how long ago (a close
// button while hovered). Swipe a line sideways, or use its close button, to
// remove it; Clear empties the list. Its button (Open a download or a drive)
// runs on a click while the shell remembers it.
//   collapsible  live activities share the island: the header opens and
//                closes the list (IslandView hides them while it's open)
Column {
    id: root

    property bool shown: false
    property bool collapsible: false
    property bool open: true
    signal toggleRequested
    property real maxHeight: 200 // what's left of the island's window
    readonly property int rowH: 36
    readonly property int most: 8
    readonly property bool listShown: !collapsible || open
    readonly property color ink: "#F5F5F7"
    readonly property color ink2: Qt.rgba(1, 1, 1, 0.6)
    readonly property bool still: Config.options.appearance.reduceMotion === true
    spacing: 4

    property real appear: shown ? 1 : 0
    Behavior on appear { NumberAnimation { duration: Math.round(280 * CoolDown.motionScale); easing.type: Easing.OutCubic } }
    opacity: appear
    transform: Translate { y: root.still ? 0 : (1 - root.appear) * 8 }

    // A clock for "5m" (only while it shows)
    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        enabled: root.visible
    }

    // Header: Recent (a toggle when it shares the island), Clear
    Item {
        width: root.width
        height: 24
        Rectangle {
            id: toggle
            x: -6
            width: toggleRow.implicitWidth + 12
            height: 24
            radius: 12
            color: !root.collapsible ? "transparent" : toggleMouse.pressed ? Qt.rgba(1, 1, 1, 0.16) : toggleMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            border.width: toggleMouse.activeFocus ? 2 : 0
            border.color: Theme.accent
            Row {
                id: toggleRow
                anchors.centerIn: parent
                spacing: 4
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.collapsible && !root.open ? "Recent · " + Math.min(root.most, Island.history.length) : "Recent"
                    color: root.ink2
                    font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.DemiBold; features: { "tnum": 1 } }
                }
                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.collapsible
                    name: root.open ? "caret-up" : "caret-down"
                    bold: true
                    size: 10
                    color: root.ink2
                }
            }
            MouseArea {
                id: toggleMouse
                anchors.fill: parent
                enabled: root.collapsible
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                activeFocusOnTab: root.collapsible
                Keys.onReturnPressed: root.toggleRequested()
                Keys.onSpacePressed: root.toggleRequested()
                onClicked: root.toggleRequested()
            }
            Accessible.role: Accessible.Button
            Accessible.name: root.open ? "Hide recent alerts" : "Show recent alerts"
        }
        Rectangle {
            anchors { right: parent.right; rightMargin: -6 }
            visible: root.listShown
            width: clearText.implicitWidth + 16
            height: 24
            radius: 12
            color: clearMouse.pressed ? Qt.rgba(1, 1, 1, 0.16) : clearMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
            border.width: clearMouse.activeFocus ? 2 : 0
            border.color: Theme.accent
            Text {
                id: clearText
                anchors.centerIn: parent
                text: "Clear"
                color: root.ink2
                font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
            }
            MouseArea {
                id: clearMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                activeFocusOnTab: true
                Keys.onReturnPressed: Island.clearHistory()
                Keys.onSpacePressed: Island.clearHistory()
                onClicked: Island.clearHistory()
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Clear recent alerts"
        }
    }

    // The lines; more than fit scroll
    Flickable {
        id: flick
        visible: root.listShown
        width: root.width
        height: Math.max(0, Math.min(lines.implicitHeight, root.maxHeight - 28))
        contentHeight: lines.implicitHeight
        interactive: contentHeight > height + 1
        flickableDirection: Flickable.VerticalFlick
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        Column {
            id: lines
            width: flick.width
            Repeater {
                model: ScriptModel { values: Island.history.slice(0, root.most) }
                Item {
                    id: line
                    required property var modelData
                    readonly property var act: Island.historyActs[modelData.hid] ?? null
                    width: lines.width
                    height: root.rowH

                    // Swiped sideways: follows the finger, then slides off
                    property real dx: 0
                    property bool leaving: false
                    function leave(dir) {
                        if (leaving)
                            return;
                        leaving = true;
                        back.stop();
                        offX.to = root.still ? 0 : dir * width;
                        off.start();
                    }
                    SequentialAnimation {
                        id: off
                        ParallelAnimation {
                            NumberAnimation { id: offX; target: line; property: "dx"; duration: Math.round(150 * CoolDown.motionScale); easing.type: Easing.InCubic }
                            NumberAnimation { target: line; property: "opacity"; to: 0; duration: Math.round(150 * CoolDown.motionScale); easing.type: Easing.InCubic }
                        }
                        ScriptAction { script: Island.forget(line.modelData.hid) }
                    }
                    NumberAnimation { id: back; target: line; property: "dx"; to: 0; duration: Theme.motion.popover; easing.type: Easing.OutCubic }
                    opacity: 1

                    HoverHandler { id: lineHover }
                    DragHandler {
                        target: null
                        yAxis.enabled: false
                        onTranslationChanged: if (active) line.dx = translation.x
                        onActiveChanged: {
                            if (active) {
                                back.stop();
                                return;
                            }
                            if (Math.abs(line.dx) > 72 || Math.abs(centroid.velocity.x) > 500)
                                line.leave(line.dx < 0 ? -1 : 1);
                            else
                                back.start();
                        }
                    }
                    TapHandler {
                        enabled: line.act !== null
                        cursorShape: Qt.PointingHandCursor
                        onTapped: line.act.act()
                    }

                    Item {
                        width: parent.width
                        height: parent.height
                        x: line.dx
                        opacity: 1 - Math.min(0.7, Math.abs(line.dx) / Math.max(1, line.width))
                        Rectangle {
                            anchors { fill: parent; leftMargin: -6; rightMargin: -6 }
                            radius: 10
                            color: Qt.rgba(1, 1, 1, lineHover.hovered ? 0.08 : 0)
                            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                        }
                        Rectangle {
                            id: badge
                            anchors.verticalCenter: parent.verticalCenter
                            width: 24
                            height: 24
                            radius: 12
                            color: Qt.alpha(line.modelData.color, 0.22)
                            Icon {
                                anchors.centerIn: parent
                                name: line.modelData.icon
                                fill: true
                                size: 14
                                color: line.modelData.color
                            }
                        }
                        Text {
                            id: what
                            anchors { left: badge.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                            width: Math.min(implicitWidth, (trail.x - x - 8) * 0.6)
                            text: line.modelData.title
                            elide: Text.ElideRight
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                        }
                        Text {
                            anchors { left: what.right; leftMargin: 6; right: trail.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            text: [line.modelData.subtitle, line.modelData.battery !== undefined ? Math.round(line.modelData.battery * 100) + "%" : "", line.modelData.times > 1 ? "×" + line.modelData.times : ""].filter(x => !!x).join(" · ")
                            elide: Text.ElideRight
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                        }
                        // How long ago; the close button while hovered
                        Item {
                            id: trail
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            width: Math.max(28, ago.implicitWidth)
                            height: 28
                            Text {
                                id: ago
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                visible: !lineHover.hovered
                                text: Island.ago(line.modelData.at, clock.date.getTime())
                                color: Qt.rgba(1, 1, 1, 0.45)
                                font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
                            }
                            Rectangle {
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                visible: lineHover.hovered
                                width: 20
                                height: 20
                                radius: 10
                                color: xMouse.pressed ? "#5A5A5E" : xMouse.containsMouse ? "#48484A" : "#3A3A3C"
                                border.width: 1
                                border.color: Qt.rgba(1, 1, 1, 0.2)
                                Icon {
                                    anchors.centerIn: parent
                                    name: "x"
                                    bold: true
                                    size: 10
                                    color: root.ink
                                }
                                MouseArea {
                                    id: xMouse
                                    anchors { fill: parent; margins: -4 }
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: line.leave(1)
                                }
                                Accessible.role: Accessible.Button
                                Accessible.name: "Remove " + line.modelData.title
                            }
                        }
                    }
                }
            }
        }
    }
}
