import QtQuick
import qs.common

// Toolbar button for the screenshot editor: icon, optional label, and a
// tooltip after a hover. Variants: ghost (tools), secondary, primary, danger.
// `selected` marks the active tool; `busy` swaps the icon for a spinner.
Rectangle {
    id: root

    property string icon
    property string label
    property string tooltip
    property string shortcut // shown in the tooltip
    property string variant: "ghost"
    property bool selected: false
    property bool busy: false
    property bool tipBelow: true
    signal clicked

    readonly property bool primary: variant === "primary"
    readonly property bool live: enabled && !busy
    readonly property color fg: !enabled ? Theme.color.tertiary : primary ? Theme.color.onAccent : variant === "danger" ? Theme.color.danger : selected ? Theme.accent : Theme.color.label

    implicitHeight: 36
    implicitWidth: label === "" ? 36 : row.implicitWidth + 28
    radius: Theme.radius.control
    color: {
        if (primary)
            return !enabled ? Theme.color.fill : mouse.pressed ? Qt.darker(Theme.accent, 1.25) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent;
        if (selected)
            return Qt.alpha(Theme.accent, mouse.pressed ? 0.28 : 0.18);
        if (!live)
            return variant === "secondary" ? Theme.color.fill : "transparent";
        if (mouse.pressed)
            return Theme.color.fillPressed;
        if (mouse.containsMouse)
            return Theme.color.fillHover;
        return variant === "secondary" ? Theme.color.fill : "transparent";
    }
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    scale: mouse.pressed && live ? 0.97 : 1
    Behavior on scale { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }

    // Keyboard focus ring
    Rectangle {
        anchors.fill: parent
        anchors.margins: -4
        radius: root.radius + 4
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
        visible: root.activeFocus
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Item {
            width: 16
            height: 16
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== "" || root.busy
            Icon {
                id: glyph
                anchors.centerIn: parent
                name: root.busy ? "circle-notch" : root.icon
                size: 16
                color: root.fg
                RotationAnimator on rotation {
                    running: root.busy
                    from: 0
                    to: 360
                    duration: 900
                    loops: Animation.Infinite
                }
                rotation: 0
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.label !== ""
            text: root.label
            color: root.fg
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.live
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
    Keys.onSpacePressed: if (live) clicked()
    Keys.onReturnPressed: if (live) clicked()
    activeFocusOnTab: true

    // Tooltip after a short hover
    Timer {
        id: tipDelay
        interval: 600
        running: mouse.containsMouse && root.tooltip !== "" && !mouse.pressed
    }
    Rectangle {
        id: tip
        visible: opacity > 0
        opacity: mouse.containsMouse && !tipDelay.running && root.tooltip !== "" ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        z: 100
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.tipBelow ? parent.height + 8 : -height - 8
        width: tipRow.implicitWidth + 16
        height: 26
        radius: 8
        color: Theme.dark ? "#3A3A3F" : "#1C1C1E"
        Row {
            id: tipRow
            anchors.centerIn: parent
            spacing: 6
            Text {
                text: root.tooltip
                color: "#F2F2F5"
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            Text {
                visible: root.shortcut !== ""
                text: root.shortcut
                color: Qt.rgba(1, 1, 1, 0.55)
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
    }
}
