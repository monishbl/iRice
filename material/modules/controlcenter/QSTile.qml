import QtQuick
import qs.common
import qs.services

// One Quick Settings tile in the Material look, as Android 16 draws them.
// Small: the icon alone, the whole tile one button. Large (two columns): the
// icon in a round button of its own at the start, which switches the tile,
// then its name and state (the network's name); the name opens the tile's
// page or choices, as on Android. On: primary (small), or primaryContainer
// with a primary icon button (large); off: surfaceContainerHighest. Shapes
// follow state, Expressive's way: an on tile rounds into a pill, an off one
// keeps 20 px corners, and a press squares it off, on the fast spatial
// spring. Info tiles (battery, CPU & memory, weather) show their reading.
// In Edit controls a click changes its size, − removes it, and it can be
// dragged (QuickSettings handles the drop). Same module definitions and
// actions as Control Center's (ModuleDefs).
Item {
    id: root

    required property string moduleId
    property var defs // ModuleDefs
    property bool large: false
    property bool editing: false
    signal openPage(string page)
    signal menuRequested(var items)
    signal removeRequested
    signal resizeRequested

    readonly property var def: defs?.all[moduleId] ?? ({ label: moduleId, kind: "action", icon: "question", sizes: ["1x1"] })
    readonly property bool info: def.kind === "info"
    // (in Edit controls every tile shows off, as Android's editor does)
    readonly property bool on: !editing && (def.on ?? false)
    readonly property bool pressed: tileMouse.pressed || iconMouse.pressed

    // Container, the content on it, and the state line's colour
    readonly property color container: on ? (large ? Theme.m3.primaryContainer : Theme.m3.primary) : Theme.m3.surfaceContainerHighest
    readonly property color ink: on ? (large ? Theme.m3.onPrimaryContainer : Theme.m3.onPrimary) : Theme.m3.onSurface
    readonly property color subInk: on ? Qt.alpha(Theme.m3.onPrimaryContainer, 0.8) : Theme.m3.onSurfaceVariant

    readonly property string iconName: moduleId === "battery" ? (Island.charging ? "battery-charging" : Island.level < 0.2 ? "battery-low" : "battery-high") : def.icon
    // Info tiles: the reading, then what it means
    readonly property string title: !info ? def.label : moduleId === "battery" ? Math.round(Island.level * 100) + "%" : moduleId === "weather" ? (Weather.ready ? Weather.temp + Weather.unit : "—") : "CPU " + Math.round(Resources.cpu * 100) + "%"
    readonly property string subtitle: !info ? (def.status ?? "") : moduleId === "battery" ? (Island.charging ? "Charging" : "On battery") : moduleId === "weather" ? (Weather.condition || Weather.place || "") : "Memory " + Math.round(Resources.memory * 100) + "%"

    // ── What a click does ───────────────────────────────────────────────
    function act() {
        if (editing)
            root.resizeRequested();
        else if (!info)
            def.act?.();
        else if (def.page)
            root.openPage(def.page);
    }
    // The name of a large tile: its choices, its page, else the switch
    function openDetail() {
        if (def.menu)
            openMenu();
        else if (def.page)
            root.openPage(def.page);
        else
            act();
    }
    // Right-click or press and hold: its choices or its page
    function secondary() {
        if (editing)
            return;
        if (def.menu)
            openMenu();
        else if (def.page)
            root.openPage(def.page);
    }
    function openMenu() {
        root.menuRequested(def.menu().map(it => it.openPage ? Object.assign({}, it, { action: () => root.openPage(it.openPage) }) : it));
    }

    // ── Keyboard ────────────────────────────────────────────────────────
    // Each tile is one stop: Tab or the arrows move between tiles, Space
    // switches it, Enter opens its page (Edit controls: changes its size),
    // the Menu key its choices, Delete removes it while editing
    activeFocusOnTab: visible
    Keys.onPressed: event => {
        const k = event.key;
        if (k === Qt.Key_Space && !event.isAutoRepeat) {
            act();
        } else if ((k === Qt.Key_Return || k === Qt.Key_Enter) && !event.isAutoRepeat) {
            if (editing)
                resizeRequested();
            else if (def.page)
                openPage(def.page);
            else
                act();
        } else if (k === Qt.Key_Menu || (k === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
            secondary();
        } else if (editing && (k === Qt.Key_Delete || k === Qt.Key_Backspace)) {
            removeRequested();
        } else if (k === Qt.Key_Left || k === Qt.Key_Right || k === Qt.Key_Up || k === Qt.Key_Down) {
            moveFocus(k === Qt.Key_Left ? -1 : k === Qt.Key_Right ? 1 : 0, k === Qt.Key_Up ? -1 : k === Qt.Key_Down ? 1 : 0);
        } else {
            return;
        }
        event.accepted = true;
    }
    // The nearest tile whose centre lies that way
    function moveFocus(dx, dy) {
        const cx = x + width / 2, cy = y + height / 2;
        let best = null, bestD = Infinity;
        for (const m of parent?.children ?? []) {
            if (m === root || m.moduleId === undefined || !m.visible)
                continue;
            const mx = m.x + m.width / 2 - cx, my = m.y + m.height / 2 - cy;
            if ((dx !== 0 && Math.sign(mx) !== dx) || (dy !== 0 && Math.sign(my) !== dy))
                continue;
            const d = dx !== 0 ? Math.abs(mx) + 2 * Math.abs(my) : Math.abs(my) + 2 * Math.abs(mx);
            if (d < bestD) {
                bestD = d;
                best = m;
            }
        }
        best?.forceActiveFocus(Qt.TabFocusReason);
    }
    Accessible.role: def.kind === "toggle" ? Accessible.CheckBox : Accessible.Button
    Accessible.name: info ? def.label + ", " + title : def.label ?? moduleId
    Accessible.description: subtitle
    Accessible.checkable: def.kind === "toggle"
    Accessible.checked: on
    Accessible.onPressAction: act()
    FocusRing {
        radius: root.corner + 5
    }

    // ── The tile ────────────────────────────────────────────────────────
    // A press (Expressive): it swells, its corners square off, and both
    // spring back with a small bounce
    // (pressed: a little squarer than at rest, not square)
    readonly property real restCorner: on ? Math.min(height, 72) / 2 : 20
    property real corner: pressed ? restCorner * 0.7 : restCorner
    Behavior on corner {
        SpringAnimation { spring: Theme.motion.springPress.spring; damping: Theme.motion.springPress.damping; epsilon: 0.25 }
    }
    HoverHandler {
        id: hover
        enabled: !root.editing
    }
    Rectangle {
        anchors.fill: parent
        radius: root.corner
        // (state layers: hover 8%, pressed 10% of the content colour)
        color: Qt.tint(root.container, Qt.alpha(root.ink, tileMouse.pressed ? 0.1 : hover.hovered && !iconMouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    // Small: the icon (with an info tile's reading under it)
    Icon {
        visible: !root.large
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: smallReading.visible ? -9 : 0
        name: root.iconName
        fill: root.on
        size: 22
        color: root.ink
    }
    Text {
        id: smallReading
        visible: !root.large && root.info && root.moduleId === "battery"
        anchors { horizontalCenter: parent.horizontalCenter; verticalCenter: parent.verticalCenter; verticalCenterOffset: 14 }
        text: root.title
        color: root.subInk
        font { family: Theme.font.family; pixelSize: 12; weight: Font.Medium; features: { "tnum": 1 } }
    }

    // Large: the icon button, the name and state, and a chevron when the
    // name opens something
    Rectangle {
        id: iconButton
        visible: root.large
        x: Math.round((root.height - height) / 2) // concentric with the pill's round end
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(root.height, 72) - 24 // (a tall tile keeps a row's icon)
        height: width
        property real corner: (root.on ? width / 2 : 16) * (iconMouse.pressed ? 0.7 : 1)
        Behavior on corner {
            SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.25 }
        }
        radius: corner
        color: root.on ? Qt.tint(Theme.m3.primary, Qt.alpha(Theme.m3.onPrimary, iconMouse.pressed ? 0.1 : iconMouse.containsMouse ? 0.08 : 0))
            : Qt.tint(root.container, Qt.alpha(Theme.m3.onSurface, iconMouse.pressed ? 0.18 : iconMouse.containsMouse ? 0.14 : 0.08))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        Icon {
            anchors.centerIn: parent
            name: root.iconName
            fill: root.on
            size: 20
            color: root.on ? Theme.m3.onPrimary : Theme.m3.onSurface
        }
    }
    Column {
        visible: root.large
        anchors { left: iconButton.right; leftMargin: 12; right: chevron.visible ? chevron.left : parent.right; rightMargin: chevron.visible ? 4 : 16; verticalCenter: parent.verticalCenter }
        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: root.ink
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.subtitle
            elide: Text.ElideRight
            color: root.subInk
            font { family: Theme.font.family; pixelSize: 12; features: { "tnum": 1 } }
        }
    }
    Icon {
        id: chevron
        visible: root.large && !root.editing && (!!root.def.menu || !!root.def.page)
        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
        name: root.def.menu ? "caret-down" : "caret-right"
        size: 16
        color: root.subInk
    }

    // Click: the tile's action (small) or its page (a large tile's name);
    // right-click or press and hold: its choices or page
    MouseArea {
        id: tileMouse
        anchors.fill: parent
        enabled: !root.editing
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        pressAndHoldInterval: 550
        property bool held: false // the press became a press and hold: no click
        onPressed: held = false
        onPressAndHold: m => {
            if (m.button === Qt.LeftButton) {
                held = true;
                root.secondary();
            }
        }
        onClicked: m => {
            if (held)
                return;
            if (m.button === Qt.RightButton)
                root.secondary();
            else if (root.large)
                root.openDetail();
            else
                root.act();
        }
    }
    // A large tile's icon button: switches it
    MouseArea {
        id: iconMouse
        visible: root.large && !root.editing
        x: iconButton.x - 6
        y: iconButton.y - 6
        width: iconButton.width + 12
        height: iconButton.height + 12
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => m.button === Qt.RightButton ? root.secondary() : root.act()
    }

    // ── Edit controls ───────────────────────────────────────────────────
    // The whole tile is the target: a click changes its size
    MouseArea {
        anchors.fill: parent
        visible: root.editing
        cursorShape: Qt.OpenHandCursor
        onClicked: root.resizeRequested()
    }
    Rectangle {
        visible: root.editing
        x: -6
        y: -6
        z: 10
        width: 24
        height: 24
        radius: 12
        color: Theme.m3.inverseSurface
        Accessible.role: Accessible.Button
        Accessible.name: "Remove " + (root.def.label ?? root.moduleId)
        Rectangle {
            anchors.centerIn: parent
            width: 10
            height: 2
            radius: 1
            color: Theme.m3.inverseOnSurface
        }
        MouseArea {
            anchors { fill: parent; margins: -4 }
            cursorShape: Qt.PointingHandCursor
            onClicked: root.removeRequested()
        }
    }
}
