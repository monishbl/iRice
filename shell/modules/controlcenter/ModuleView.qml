import QtQuick
import QtQuick.Effects
import Quickshell.Services.UPower
import qs.common
import qs.glass
import qs.services

// One Control Center module at its size. 1×1: a round button. 2×1: a pill
// with the button, title and status. 2×2: a card with iOS corners (half a
// 1×1, continuous). Sliders are the whole module: a 4×1 or 1×2 capsule.
// 4×1 media: a wide row. In edit mode it shows a remove badge, cycles its size on click
// and can be dragged (the grid handles the drop).
Item {
    id: root

    required property string moduleId
    required property int cols
    required property int rows
    property var defs // ModuleDefs
    property bool editing: false
    property bool preview: false // drawn in the gallery: no actions
    property bool chip: false // its own glass (Control Center without a panel)
    // While a detail popover is open: the control it belongs to stays sharp
    // and lifts a little; the others blur and fade back
    property bool dimmed: false
    property bool focused: false
    property real dim: dimmed ? 1 : 0
    Behavior on dim { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
    opacity: 1 - 0.45 * dim
    layer.enabled: dim > 0.01
    layer.effect: MultiEffect {
        blurEnabled: true
        blur: 0.85 * root.dim
        blurMax: 24
        saturation: -0.3 * root.dim
    }
    scale: focused ? 1.04 : 1
    Behavior on scale { SpringAnimation { spring: Theme.motion.spring + 1; damping: Theme.motion.damping + 0.04; epsilon: 0.002 } }
    signal openPage(string page)
    signal removeRequested
    signal resizeRequested

    // The grid's cell: a 1×1 module's size (cards round their corners to half
    // of it, like iOS, so every curve matches the round buttons')
    property real unit: (width - (cols - 1) * 10) / Math.max(1, cols)
    readonly property var def: defs?.all[moduleId] ?? ({ label: moduleId, kind: "action", icon: "question", sizes: ["1x1"] })
    readonly property string size: cols + "x" + rows
    readonly property bool round: size === "1x1" && (def.kind === "toggle" || def.kind === "action")
    // 2×1 toggles, actions and info tiles are pills (round ends), like iOS
    readonly property bool pill: rows === 1 && cols === 2 && (def.kind === "toggle" || def.kind === "action" || def.kind === "info")
    readonly property real shapeRadius: round || pill || def.kind === "slider" ? Math.min(width, height) / 2 : Math.min(unit / 2, Math.min(width, height) / 2)
    // On: a white circle with the glyph in the accent colour (darkened if
    // the accent is too light to read on white)
    readonly property color ink: Theme.accentOnGlyph
    readonly property bool on: def.on ?? false
    // A module with choices (def.menu: items for ContextMenu) offers them from
    // the pill's text and on right-click or press and hold; an item with
    // `openPage` opens the module's page
    PointerMenu { id: choices }
    function openMenu() {
        choices.open(def.menu().map(it => it.openPage ? Object.assign({}, it, { action: () => root.openPage(it.openPage) }) : it));
    }

    // ── Keyboard ────────────────────────────────────────────────────────
    // Each module is one stop: Tab or the arrows move between modules (to the
    // nearest one in that direction), Space switches a toggle or runs an
    // action, Enter (or the Menu key) opens its page. Sliders are their own
    // stops and take the arrows themselves.
    readonly property bool ownKeys: def.kind === "slider"
    activeFocusOnTab: !ownKeys && !preview && visible
    Keys.onPressed: event => {
        const k = event.key;
        if ((k === Qt.Key_Space) && !event.isAutoRepeat) {
            if (def.kind === "toggle" || def.kind === "action")
                act();
            else if (def.page)
                openPage(def.page);
        } else if ((k === Qt.Key_Return || k === Qt.Key_Enter) && !event.isAutoRepeat) {
            if (editing)
                resizeRequested();
            else if (def.page)
                openPage(def.page);
            else
                act();
        } else if (k === Qt.Key_Menu || (k === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
            if (def.page && !editing)
                openPage(def.page);
        } else if (k === Qt.Key_Left || k === Qt.Key_Right || k === Qt.Key_Up || k === Qt.Key_Down) {
            moveFocus(k === Qt.Key_Left ? -1 : k === Qt.Key_Right ? 1 : 0, k === Qt.Key_Up ? -1 : k === Qt.Key_Down ? 1 : 0);
        } else {
            return;
        }
        event.accepted = true;
    }
    // The nearest module whose centre lies that way (rows first for up and
    // down, columns for left and right)
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
        best?.takeFocus();
    }
    function takeFocus() {
        const t = ownKeys ? (slider.visible ? slider : nextItemInFocusChain(true)) : root;
        t?.forceActiveFocus(Qt.TabFocusReason);
    }
    Accessible.role: def.kind === "toggle" ? Accessible.CheckBox : def.kind === "slider" ? Accessible.Slider : Accessible.Button
    Accessible.name: def.label ?? moduleId
    Accessible.description: def.status ?? ""
    Accessible.checkable: def.kind === "toggle"
    Accessible.checked: on
    Accessible.onPressAction: act()
    FocusRing {
        radius: root.shapeRadius + 4
    }

    function act() {
        if (editing) {
            root.resizeRequested();
            return;
        }
        if (!preview)
            def.act?.();
    }

    // As a chip: liquid glass of its own
    Glass {
        id: chipGlass
        visible: root.chip
        anchors.fill: parent
        anchors.margins: -shadowMargin
        shadowMargin: 12
        radius: root.shapeRadius
    }
    // Background: a circle for 1×1 buttons, a platter otherwise, with
    // continuous (squircle) corners like the glass. Sliders draw their own.
    Squircle {
        visible: root.def.kind !== "slider"
        anchors.fill: parent
        radius: root.shapeRadius
        readonly property color idle: root.chip ? Qt.alpha("white", Theme.dark ? 0.07 : 0.28) : Theme.color.platter
        color: root.round ? (root.on ? Theme.accentOnFill : bgMouse.containsMouse ? Theme.color.fillHover : idle) : idle
        borderWidth: 1
        borderColor: Qt.alpha("white", Theme.dark ? 0.06 : 0.3)
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    // ── 1×1 toggles and actions: just the icon ──────────────────────────
    Icon {
        visible: root.round
        anchors.centerIn: parent
        name: root.def.icon
        fill: root.on
        size: Math.round(root.width * 0.36)
        color: root.on ? root.ink : Theme.color.label
    }

    // ── 2×1 / 2×2 toggles and actions: button, title, status ────────────
    Item {
        visible: !root.round && (root.def.kind === "toggle" || root.def.kind === "action")
        anchors.fill: parent
        CircleToggle {
            id: btn
            x: root.pill ? Math.round((root.height - size) / 2) : 10 // concentric with the pill's round end
            anchors.verticalCenter: root.rows === 1 ? parent.verticalCenter : undefined
            y: root.rows === 1 ? 0 : 14
            size: Math.min(40, root.height - 20)
            icon: root.def.icon
            checked: root.on
            focusable: false // the module is the keyboard stop
            onClicked: root.act()
        }
        Column {
            anchors { left: root.rows === 1 ? btn.right : parent.left; leftMargin: root.rows === 1 ? 10 : 14; right: parent.right; rightMargin: 10 }
            anchors.verticalCenter: root.rows === 1 ? parent.verticalCenter : undefined
            y: root.rows === 1 ? 0 : parent.height - height - 14
            Text {
                width: parent.width
                text: root.def.label
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                visible: (root.def.status ?? "") !== ""
                text: root.def.status ?? ""
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
    }

    // ── Sliders (4×1 wide, 1×2 tall): the module is the capsule ──────────
    // Icon only; right-click or press and hold opens its page (Sound)
    CapsuleSlider {
        id: slider
        visible: root.def.kind === "slider"
        accessibleName: root.def.label ?? root.moduleId
        anchors.fill: parent
        icon: root.def.icon
        value: root.moduleId === "display" ? Brightness.value : (Audio.muted ? 0 : Audio.volume)
        hasMore: !!root.def.page && !root.editing
        onMoved: v => { if (root.preview || root.editing) return; root.moduleId === "display" ? Brightness.set(v) : Audio.setVolume(v); }
        onMore: root.openPage(root.def.page)
        enabled: !root.editing && !root.preview
    }

    // ── Now Playing ─────────────────────────────────────────────────────
    Item {
        visible: root.def.kind === "media"
        anchors.fill: parent
        readonly property var player: Media.player
        // Album art, nested in the module's corner: radius = outer - inset
        Item {
            id: art
            readonly property real inset: root.rows === 2 ? 12 : 10
            readonly property real radius: Math.max(6, root.shapeRadius - inset)
            x: inset
            y: root.rows === 2 ? 12 : (parent.height - height) / 2
            width: root.rows === 2 ? 56 : parent.height - 20
            height: width
            Squircle {
                anchors.fill: parent
                radius: art.radius
                color: Theme.color.fill
            }
            Image {
                id: artImage
                anchors.fill: parent
                source: parent.parent.player?.trackArtUrl ?? ""
                fillMode: Image.PreserveAspectCrop
                sourceSize: Qt.size(112, 112)
                asynchronous: true
                layer.enabled: status === Image.Ready
                layer.effect: MultiEffect {
                    maskEnabled: true
                    maskSource: artMask
                    maskThresholdMin: 0.5
                    maskSpreadAtMin: 1
                }
            }
            Squircle {
                id: artMask
                anchors.fill: parent
                radius: art.radius
                color: "black"
                visible: false
                layer.enabled: true
            }
            Icon {
                anchors.centerIn: parent
                visible: !(parent.parent.player?.trackArtUrl)
                name: "music-notes"
                size: 22
                color: Theme.color.tertiary
            }
        }
        Column {
            anchors { left: root.rows === 2 ? parent.left : art.right; leftMargin: root.rows === 2 ? 12 : 10; right: root.rows === 2 ? parent.right : mediaControls.left; rightMargin: 10 }
            y: root.rows === 2 ? art.y + art.height + 8 : (parent.height - height) / 2
            Text {
                width: parent.width
                text: parent.parent.player?.trackTitle || "Not Playing"
                elide: Text.ElideRight
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
            }
            Text {
                width: parent.width
                text: parent.parent.player?.trackArtist ?? ""
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption }
            }
        }
        // To a Chromecast or Google TV: the Cast page (like AirPlay's
        // corner button on a Mac)
        IconButton {
            visible: (Cast.devices.length > 0 || Cast.active) && root.rows === 2
            anchors { right: parent.right; top: parent.top; margins: 4 }
            width: 28
            height: 28
            icon: "screencast"
            fill: Cast.active
            iconSize: 15
            iconColor: Cast.active ? Theme.accent : Theme.color.secondary
            tooltip: Cast.active ? "Casting to " + (Cast.device?.name ?? "") : "Cast to a TV"
            enabled: !root.editing && !root.preview
            onClicked: Panels.ccPage = "cast"
        }
        Row {
            id: mediaControls
            anchors { right: root.rows === 2 ? undefined : parent.right; rightMargin: 6; horizontalCenter: root.rows === 2 ? parent.horizontalCenter : undefined }
            y: root.rows === 2 ? parent.height - height - 6 : (parent.height - height) / 2
            spacing: 0
            enabled: !root.editing && !root.preview && !!parent.player
            Repeater {
                model: [
                    { icon: "skip-back", act: () => Media.player?.previous() },
                    { icon: Media.player?.isPlaying ? "pause" : "play", act: () => Media.player?.togglePlaying() },
                    { icon: "skip-forward", act: () => Media.player?.next() }
                ]
                IconButton {
                    required property var modelData
                    width: 32
                    height: 32
                    icon: modelData.icon
                    fill: true
                    iconSize: 16
                    iconColor: Theme.color.label
                    onClicked: modelData.act()
                }
            }
        }
    }

    // ── Info cards ──────────────────────────────────────────────────────
    Item {
        visible: root.def.kind === "info"
        anchors { fill: parent; margins: root.pill ? Math.round((root.height - infoIcon.height) / 2) : 12 }
        // In a pill, the icon sits in a circle like the toggles' (concentric
        // with the round end), so the row of pills reads as one set
        Rectangle {
            id: infoIcon
            visible: root.size === "1x1" || root.rows === 1
            anchors.verticalCenter: parent.verticalCenter
            width: root.pill ? Math.min(40, root.height - 20) : 20
            height: width
            radius: width / 2
            color: root.pill ? Theme.color.toggleOff : "transparent"
            Icon {
                anchors.centerIn: parent
                name: root.moduleId === "battery" ? (Island.charging ? "battery-charging" : "battery-high") : root.def.icon
                size: root.pill ? Math.round(infoIcon.width * 0.5) : 20
                color: Theme.color.label
            }
        }
        Column {
            visible: root.size !== "1x1"
            anchors { left: infoIcon.visible ? infoIcon.right : parent.left; leftMargin: infoIcon.visible ? 10 : 0; right: parent.right; verticalCenter: root.rows === 1 ? parent.verticalCenter : undefined }
            spacing: 2
            Text {
                width: parent.width
                text: root.moduleId === "battery" ? Math.round(Island.level * 100) + "%" : root.moduleId === "weather" ? (Weather.ready ? Weather.temp + Weather.unit : "—") : "CPU " + Math.round(Resources.cpu * 100) + "%"
                color: Theme.color.label
                font { family: Theme.font.family; pixelSize: root.rows === 2 ? Theme.font.large : Theme.font.small; weight: Font.DemiBold; features: { "tnum": 1 } }
            }
            Text {
                width: parent.width
                text: root.moduleId === "battery" ? (Island.charging ? "Charging" : "On battery") : root.moduleId === "weather" ? (Weather.condition || Weather.place) : "Memory " + Math.round(Resources.memory * 100) + "%"
                elide: Text.ElideRight
                color: Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
            }
        }
        Text {
            visible: root.size === "1x1"
            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom }
            text: root.moduleId === "battery" ? Math.round(Island.level * 100) + "%" : ""
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption; features: { "tnum": 1 } }
        }
    }

    // Click: act (normal), cycle size (editing). Right-click a 1×1 for its page.
    MouseArea {
        id: bgMouse
        anchors.fill: parent
        z: -1
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        enabled: root.def.kind !== "slider" || root.editing
        // Press and hold, like right-click: the module's page
        onPressAndHold: if (!root.editing && !root.preview) root.def.menu ? root.openMenu() : root.def.page ? root.openPage(root.def.page) : undefined
        onClicked: m => {
            if (root.def.menu && !root.editing && !root.preview && (m.button === Qt.RightButton || !root.round))
                root.openMenu();
            else if (m.button === Qt.RightButton && root.def.page && !root.editing)
                root.openPage(root.def.page);
            else if (root.editing)
                root.resizeRequested();
            else if (!root.round && root.def.page)
                root.openPage(root.def.page);
            else
                root.act();
        }
    }
    // In edit mode nothing inside reacts; the whole module is the target
    MouseArea {
        anchors.fill: parent
        visible: root.editing
        onClicked: root.resizeRequested()
    }

    // Remove badge
    Rectangle {
        visible: root.editing && !root.preview
        x: -6
        y: -6
        z: 10
        width: 20
        height: 20
        radius: 10
        color: Theme.dark ? "#3a3a40" : "#E4E4E7"
        border.width: 1
        border.color: Theme.color.separator
        Rectangle {
            anchors.centerIn: parent
            width: 9
            height: 2
            radius: 1
            color: Theme.color.label
        }
        MouseArea {
            anchors { fill: parent; margins: -4 }
            onClicked: root.removeRequested()
        }
    }

    ElasticControl {
        id: elastic
        pressed: bgMouse.pressed
    }
    transform: root.preview ? [] : elastic.effects

    // Chip glass is drawn by the compositor from its position: follow the
    // press squish and drag, the focus lift and the dim fade
    Connections {
        target: elastic
        enabled: root.chip
        function onPxChanged() { chipGlass.updatePosition(); }
        function onPyChanged() { chipGlass.updatePosition(); }
        function onSChanged() { chipGlass.updatePosition(); }
    }
    onScaleChanged: if (chip) chipGlass.updatePosition()
    onDimChanged: if (chip) chipGlass.updatePosition()
}
