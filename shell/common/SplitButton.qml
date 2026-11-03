import QtQuick
import QtQuick.Effects

// Material 3 Expressive's split button: the main action, and beside it (2 px
// apart) a narrow button that opens more choices. Outer corners round,
// inner ones small; while the menu is open the narrow one turns fully round
// and its chevron flips. The menu opens below (or above, `upward`), inside
// the window.
//   SplitButton { text: "Save"; icon: "floppy-disk"; onClicked: save()
//                 menu: [{ text: "Save as a copy", icon: "files", action: () => saveCopy() }] }
Item {
    id: root

    property string text
    property string icon
    property bool tonal: false // tonal (secondaryContainer) instead of filled (primary)
    property bool busy: false
    property bool upward: false
    property var menu: [] // [{ text, icon?, action, enabled?, danger? }]
    property string tooltip
    signal clicked

    readonly property color fill: tonal ? Theme.m3.secondaryContainer : Theme.m3.primary
    readonly property color ink: tonal ? Theme.m3.onSecondaryContainer : Theme.m3.onPrimary
    property bool open: false
    // (the menu lives on the window's top item, over everything in it)
    property Item host: root
    onOpenChanged: if (open)
        menuBox.at = root.mapToItem(host, 0, 0)
    Component.onCompleted: {
        let p = root;
        while (p.parent)
            p = p.parent;
        host = p;
    }
    // (room the menu needs beyond the button: a window too small for it can
    // grow by this while it's open)
    readonly property real menuHeight: menuBox.height + 8

    implicitHeight: 40
    implicitWidth: lead.width + 2 + trail.width
    opacity: enabled ? 1 : 0.38

    // The main action
    Rectangle {
        id: lead
        width: leadRow.implicitWidth + 32
        height: root.height
        readonly property real inner: leadMouse.pressed ? 8 : 4
        topLeftRadius: height / 2
        bottomLeftRadius: height / 2
        topRightRadius: inner
        bottomRightRadius: inner
        color: Qt.tint(root.fill, Qt.alpha(root.ink, leadMouse.pressed ? 0.1 : leadMouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        transform: leadPress.effects
        ElasticControl { id: leadPress; pressed: leadMouse.pressed }
        Row {
            id: leadRow
            anchors.centerIn: parent
            spacing: 8
            Icon {
                visible: root.icon !== "" && !root.busy
                anchors.verticalCenter: parent.verticalCenter
                name: root.icon
                size: 18
                color: root.ink
            }
            Spinner {
                visible: root.busy
                anchors.verticalCenter: parent.verticalCenter
                size: 20
                color: root.ink
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.text
                color: root.ink
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
        }
        MouseArea {
            id: leadMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            enabled: !root.busy
            onClicked: root.clicked()
        }
        activeFocusOnTab: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                root.clicked();
                e.accepted = true;
            } else if (e.key === Qt.Key_Down && root.menu.length > 0) {
                root.open = true;
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: root.text
        Accessible.description: root.tooltip
        FocusRing {
            radius: lead.height / 2 + 4
        }
    }

    // More choices
    Rectangle {
        id: trail
        x: lead.width + 2
        width: 40
        height: root.height
        readonly property real inner: root.open ? height / 2 : trailMouse.pressed ? 8 : 4
        topLeftRadius: inner
        bottomLeftRadius: inner
        topRightRadius: height / 2
        bottomRightRadius: height / 2
        Behavior on topLeftRadius { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.1 } }
        Behavior on bottomLeftRadius { SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.1 } }
        color: Qt.tint(root.fill, Qt.alpha(root.ink, trailMouse.pressed ? 0.1 : trailMouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        transform: trailPress.effects
        ElasticControl { id: trailPress; pressed: trailMouse.pressed }
        Icon {
            anchors.centerIn: parent
            name: "caret-down"
            size: 18
            color: root.ink
            rotation: (root.open ? 180 : 0) + (root.upward ? 180 : 0)
            Behavior on rotation { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.OutCubic } }
        }
        MouseArea {
            id: trailMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.open = !root.open
        }
        activeFocusOnTab: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Return || e.key === Qt.Key_Enter || e.key === Qt.Key_Down) {
                root.open = !root.open;
                e.accepted = true;
            }
        }
        Accessible.role: Accessible.ButtonMenu
        Accessible.name: "More " + root.text.toLowerCase() + " options"
        FocusRing {
            radius: trail.height / 2 + 4
        }
    }

    // The menu (a click outside it, or Escape, closes it)
    MouseArea {
        visible: root.open
        parent: root.host
        anchors.fill: parent
        z: 999
        onClicked: root.open = false
    }
    Rectangle {
        id: menuBox
        visible: opacity > 0.01
        opacity: root.open ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: root.open ? Theme.motion.popover : Theme.motion.hover } }
        parent: root.host
        z: 1000
        // (where the button is, measured as the menu opens)
        property point at: Qt.point(0, 0)
        width: Math.max(200, menuCol.implicitWidth)
        height: menuCol.implicitHeight + 16
        x: Math.max(8, Math.min((parent?.width ?? 0) - width - 8, at.x + root.width - width))
        y: root.upward ? at.y - height - 8 : at.y + root.height + 8
        transform: Translate { y: (root.open ? 0 : 6) * (root.upward ? 1 : -1) }
        radius: 16
        color: Theme.m3.surfaceContainer
        RectangularShadow {
            anchors.fill: parent
            z: -1
            radius: parent.radius
            offset.y: 4
            blur: 16
            color: Qt.alpha(Theme.m3.shadow, Theme.dark ? 0.4 : 0.16)
        }
        Column {
            id: menuCol
            y: 8
            width: parent.width
            Repeater {
                model: root.open ? root.menu : []
                Rectangle {
                    id: item
                    required property var modelData
                    width: menuCol.width
                    height: 44
                    readonly property bool can: modelData.enabled !== false
                    color: itemMouse.containsMouse && can ? Qt.alpha(Theme.m3.onSurface, itemMouse.pressed ? 0.1 : 0.08) : "transparent"
                    opacity: can ? 1 : 0.38
                    Row {
                        x: 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 12
                        Icon {
                            visible: (item.modelData.icon ?? "") !== ""
                            anchors.verticalCenter: parent.verticalCenter
                            name: item.modelData.icon ?? ""
                            size: 20
                            color: item.modelData.danger ? Theme.m3.error : Theme.m3.onSurfaceVariant
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: item.modelData.text
                            color: item.modelData.danger ? Theme.m3.error : Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 14 }
                        }
                    }
                    MouseArea {
                        id: itemMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: item.can
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.open = false;
                            item.modelData.action?.();
                        }
                    }
                    Accessible.role: Accessible.MenuItem
                    Accessible.name: item.modelData.text
                    Accessible.onPressAction: itemMouse.clicked(null)
                }
            }
        }
        Keys.onEscapePressed: root.open = false
    }
}
