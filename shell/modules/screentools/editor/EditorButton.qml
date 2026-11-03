import QtQuick
import qs.common

// Toolbar button for the screenshot editor: icon, optional label, and a
// tooltip after a hover. Variants: ghost (tools), secondary, primary, danger.
// `selected` marks the active tool; `busy` swaps the icon for a spinner.
// Material: Material 3 Expressive buttons: round, squaring off while pressed;
// primary filled, secondary tonal (secondaryContainer), ghost a text or
// standard icon button; a selected toggle squares off in secondaryContainer
// (on a vibrant toolbar, `vibrant`: in primary).
Squircle {
    id: root

    property string icon
    property string label
    property string tooltip
    property string shortcut // shown in the tooltip
    property string variant: "ghost"
    property bool selected: false
    property bool busy: false
    property bool tipBelow: true
    property bool vibrant: false // (Material) sits on a primaryContainer toolbar
    signal clicked

    readonly property bool primary: variant === "primary"
    readonly property bool live: enabled && !busy
    readonly property color fg: Theme.material ? materialFg : !enabled ? Theme.color.tertiary : primary ? Theme.color.onAccent : variant === "danger" ? Theme.color.danger : selected ? Theme.accent : Theme.color.label

    // ── Material's colours: content, container, and the state layer over it ──
    readonly property color materialFg: {
        if (!Theme.material)
            return "transparent";
        if (!enabled)
            return Qt.alpha(Theme.m3.onSurface, 0.38);
        if (primary)
            return Theme.m3.onPrimary;
        if (variant === "danger")
            return Theme.m3.error;
        if (selected)
            return vibrant ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer;
        if (vibrant)
            return Theme.m3.onPrimaryContainer;
        if (variant === "secondary")
            return Theme.m3.onSecondaryContainer;
        // a ghost with a label is a text button (primary), without one a
        // standard icon button
        return label !== "" ? Theme.m3.primary : Theme.m3.onSurfaceVariant;
    }
    readonly property color materialColor: {
        if (!Theme.material)
            return "transparent";
        const layer = !live ? 0 : mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0;
        let base = "transparent";
        if (primary || variant === "secondary" || selected) {
            if (!enabled)
                return Qt.alpha(Theme.m3.onSurface, 0.12);
            base = primary || (selected && vibrant) ? Theme.m3.primary : Theme.m3.secondaryContainer;
            return Qt.tint(base, Qt.alpha(materialFg, layer));
        }
        return Qt.alpha(materialFg, layer);
    }

    implicitHeight: Theme.material ? 40 : 36
    implicitWidth: label === "" ? implicitHeight : row.implicitWidth + (Theme.material ? 32 : 28)
    // (Material: round; a selected toggle squares off, a press more so)
    transform: Theme.material ? pressFx1.effects : []
    ElasticControl { id: pressFx1; pressed: mouse.pressed; enabled: Theme.material }
    radius: Theme.material ? (mouse.pressed && live ? 8 : selected ? 12 : height / 2) : Theme.radius.control
    Behavior on radius {
        enabled: Theme.material
        MaterialSpring { speed: "fast"; epsilon: 0.25 }
    }
    color: {
        if (Theme.material)
            return materialColor;
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
    scale: mouse.pressed && live && !Theme.material ? 0.97 : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press; easing.type: Easing.OutCubic } }

    // Keyboard focus ring
    // (Material: 3 px in secondary, 2 px out)
    Squircle {
        anchors.fill: parent
        anchors.margins: Theme.material ? -5 : -4
        radius: root.radius + (Theme.material ? 5 : 4)
        color: "transparent"
        border.width: Theme.material ? 3 : 2
        border.color: Theme.material ? Theme.m3.secondary : Theme.accent
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
                visible: !(Theme.material && root.busy)
                name: root.busy ? "circle-notch" : root.icon
                fill: Theme.material && root.selected
                size: Theme.material ? 18 : 16
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
            // (Material: its loading indicator)
            Loader {
                anchors.centerIn: parent
                active: Theme.material && root.busy
                sourceComponent: Spinner {
                    size: 20
                    color: root.fg
                }
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.label !== ""
            text: root.label
            color: root.fg
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.body; weight: Font.Medium }
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
    Accessible.role: Accessible.Button
    Accessible.name: label || tooltip || icon
    Accessible.onPressAction: if (live) clicked()

    // Tooltip after a short hover
    Timer {
        id: tipDelay
        interval: 600
        running: mouse.containsMouse && root.tooltip !== "" && !mouse.pressed
    }
    Squircle {
        id: tip
        visible: opacity > 0
        opacity: mouse.containsMouse && !tipDelay.running && root.tooltip !== "" ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        z: 100
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.tipBelow ? parent.height + 8 : -height - 8
        width: tipRow.implicitWidth + 16
        height: Theme.material ? 24 : 26
        // (Material: a plain tooltip, inverseSurface, corner 4)
        radius: Theme.material ? 4 : 8
        color: Theme.material ? Theme.m3.inverseSurface : Theme.dark ? "#3A3A3F" : "#1C1C1E"
        Row {
            id: tipRow
            anchors.centerIn: parent
            spacing: 6
            Text {
                text: root.tooltip
                color: Theme.material ? Theme.m3.inverseOnSurface : "#F2F2F5"
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
            }
            Text {
                visible: root.shortcut !== ""
                text: root.shortcut
                color: Theme.material ? Qt.alpha(Theme.m3.inverseOnSurface, 0.7) : Qt.rgba(1, 1, 1, 0.55)
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
    }
}
