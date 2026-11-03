import QtQuick
import qs.common

// Text button for Settings pages: neutral fill, `primary` (accent) for the
// one main action, `danger` for removing. Keyboard: Tab to it, Space or Enter.
// Material: a 40 px pill that squares off while pressed: filled primary for
// the main action, tonal (secondaryContainer) for the rest, `minor` a text
// button, `danger` in the error container.
Squircle {
    id: root

    property string text
    property string icon
    property bool primary: false
    property bool danger: false
    property bool minor: false // (Material: a text button, for small asides)
    property bool busy: false
    signal clicked

    readonly property bool active: enabled && !busy
    // Material's container and content colours (read only in that look)
    readonly property color m3Container: !Theme.material ? "transparent" : !enabled ? Qt.alpha(Theme.m3.onSurface, 0.12) : primary ? Theme.m3.primary : danger ? Theme.m3.errorContainer : minor ? "transparent" : Theme.m3.secondaryContainer
    readonly property color m3Content: !Theme.material ? "transparent" : !enabled ? Qt.alpha(Theme.m3.onSurface, 0.38) : primary ? Theme.m3.onPrimary : danger ? Theme.m3.onErrorContainer : minor ? Theme.m3.primary : Theme.m3.onSecondaryContainer
    readonly property real m3Layer: mouse.pressed ? 0.1 : mouse.containsMouse ? 0.08 : 0
    implicitWidth: inner.implicitWidth + (Theme.material ? (minor ? 24 : 32) : 24)
    implicitHeight: Theme.material ? 40 : 28
    transform: pressFx1.effects
    ElasticControl { id: pressFx1; pressed: mouse.pressed }
    radius: Theme.material ? (mouse.pressed ? 8 : height / 2) : Theme.radius.control
    Behavior on radius {
        enabled: Theme.material
        SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping }
    }
    opacity: Theme.material || enabled ? 1 : 0.45
    activeFocusOnTab: true
    // (a text button's state layer is the label's colour on nothing)
    color: Theme.material ? (minor ? Qt.alpha(m3Content, m3Layer) : Qt.tint(m3Container, Qt.alpha(m3Content, m3Layer)))
        : primary ? (mouse.pressed ? Qt.darker(Theme.accent, 1.2) : mouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent) : mouse.pressed ? Theme.color.fillPressed : mouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
    scale: mouse.pressed && !Theme.material ? 0.97 : 1
    Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
    Behavior on color { ColorAnimation { duration: Theme.motion.hover } }

    Keys.onPressed: event => {
        if (active && !event.isAutoRepeat && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.clicked();
            event.accepted = true;
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: text
    // (the settings row it's in: "Use" or "Remove" alone don't say what for)
    Accessible.description: {
        let p = parent;
        for (let i = 0; p && i < 4; i++, p = p.parent)
            if (typeof p.label === "string" && p.label !== "")
                return p.label;
        return "";
    }
    Accessible.onPressAction: if (active) root.clicked()

    FocusRing {
        radius: root.radius + 4
    }

    // (not a Row: in Settings pages Row is the settings row)
    Item {
        id: inner
        anchors.centerIn: parent
        implicitWidth: (glyph.visible ? glyph.implicitWidth + (Theme.material ? 8 : 6) : 0) + label.implicitWidth
        implicitHeight: label.implicitHeight
        width: implicitWidth
        height: implicitHeight
        Icon {
            id: glyph
            visible: root.icon !== "" || root.busy
            anchors.verticalCenter: parent.verticalCenter
            name: root.busy ? "circle-notch" : root.icon
            bold: !Theme.material
            size: Theme.material ? 18 : 12
            color: label.color
            opacity: Theme.material && root.busy ? 0 : 1
            RotationAnimation on rotation {
                running: root.busy && !Theme.material
                from: 0
                to: 360
                duration: 900
                loops: Animation.Infinite
            }
        }
        // (Material: the loading indicator in the icon's place)
        Loader {
            active: Theme.material && root.busy
            anchors.centerIn: glyph
            sourceComponent: Spinner {
                size: 18
                color: root.m3Content
            }
        }
        Text {
            id: label
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            text: root.text
            color: Theme.material ? root.m3Content : root.primary ? Theme.color.onAccent : root.danger ? Theme.color.danger : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Theme.material ? Font.Medium : root.primary ? Font.DemiBold : Font.Medium; letterSpacing: Theme.material ? 0.1 : 0 }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.active
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            root.forceActiveFocus();
            root.clicked();
        }
    }
}
