import QtQuick
import QtQuick.Effects

// Material 3 Expressive's FAB menu: a floating action button that opens
// into a column of labelled pills above it (in primary), the button itself
// turning into a round close button. A click outside, Escape or a choice
// closes it. Anchor it at the content's bottom right; the pills open
// upward, right-aligned with it.
//   FabMenu { icon: "plus"; items: [{ text: "New note", icon: "note", action: () => … }] }
Item {
    id: root

    property string icon: "plus"
    property string label // its accessible name
    property var items: [] // [{ text, icon, action }]
    property bool open: false
    // (the menu lives on the window's top item, over everything in it)
    property Item host: root
    onOpenChanged: if (open)
        pills.at = root.mapToItem(host, 0, 0)
    Component.onCompleted: {
        let p = root;
        while (p.parent)
            p = p.parent;
        host = p;
    }

    implicitWidth: 56
    implicitHeight: 56

    // A click outside closes it
    MouseArea {
        visible: root.open
        parent: root.host
        anchors.fill: parent
        z: 998
        onClicked: root.open = false
    }

    // The pills, rising one after another
    Column {
        id: pills
        parent: root.host
        z: 999
        // (where the button is, measured as the menu opens)
        property point at: Qt.point(0, 0)
        x: at.x + root.width - width
        y: at.y - height - 8
        spacing: 4
        visible: root.open || fading.running
        Repeater {
            model: root.items
            Item {
                id: pill
                required property var modelData
                required property int index
                width: pillRow.implicitWidth + 40
                height: 56
                x: pills.width - width
                // (the nearest the button first, 30 ms apart)
                property bool out: false
                Timer {
                    running: root.open !== pill.out
                    interval: root.open ? (root.items.length - 1 - pill.index) * 30 : 0
                    onTriggered: pill.out = root.open
                }
                property real t: out ? 1 : 0
                Behavior on t {
                    SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping + 0.1; epsilon: 0.005 }
                }
                opacity: Math.min(1, t)
                transform: [
                    Scale { origin.x: pill.width; origin.y: pill.height / 2; xScale: 0.6 + 0.4 * pill.t; yScale: xScale },
                    Translate { y: (1 - pill.t) * 12 }
                ]
                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, pillMouse.pressed ? 0.1 : pillMouse.containsMouse ? 0.08 : 0))
                }
                Row {
                    id: pillRow
                    anchors.centerIn: parent
                    spacing: 12
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: pill.modelData.icon ?? ""
                        size: 22
                        color: Theme.m3.onPrimary
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: pill.modelData.text
                        color: Theme.m3.onPrimary
                        font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                    }
                }
                MouseArea {
                    id: pillMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.open = false;
                        pill.modelData.action?.();
                    }
                }
                Accessible.role: Accessible.MenuItem
                Accessible.name: pill.modelData.text
                Accessible.onPressAction: pillMouse.clicked(null)
            }
        }
    }
    Timer {
        id: fading
        interval: 300
        running: !root.open
    }

    // The button: a rounded square (primaryContainer), round and primary
    // while open, its plus turning into a close
    Rectangle {
        id: fab
        anchors.fill: parent
        z: 1000
        property real corner: root.open ? width / 2 : fabMouse.pressed ? 20 : 16
        Behavior on corner { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.1 } }
        radius: corner
        color: root.open ? Theme.m3.primary : Qt.tint(Theme.m3.primaryContainer, Qt.alpha(Theme.m3.onPrimaryContainer, fabMouse.pressed ? 0.1 : fabMouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        transform: fabPress.effects
        ElasticControl { id: fabPress; pressed: fabMouse.pressed }
        RectangularShadow {
            anchors.fill: parent
            z: -1
            radius: parent.radius
            offset.y: 3
            blur: 10
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.18)
        }
        Icon {
            anchors.centerIn: parent
            name: root.open ? "x" : root.icon
            size: 24
            color: root.open ? Theme.m3.onPrimary : Theme.m3.onPrimaryContainer
            rotation: root.open ? 90 : 0
            Behavior on rotation { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.5 } }
        }
        MouseArea {
            id: fabMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.open = !root.open
        }
        activeFocusOnTab: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                root.open = !root.open;
                e.accepted = true;
            } else if (e.key === Qt.Key_Escape && root.open) {
                root.open = false;
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.ButtonMenu
        Accessible.name: root.label
        FocusRing {
            radius: fab.radius + 4
        }
    }
}
