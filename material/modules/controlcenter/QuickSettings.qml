import QtQuick
import Quickshell
import Quickshell.Io
import qs.common
import qs.glass
import qs.services
import qs.modules.network

// Control Center in the Material look: Android 16's Quick Settings, in the
// same place (under the menu bar's right end), opened by the same shortcut,
// IPC and menu bar item, with the same controls (ModuleDefs) and pages.
// From the top: the brightness and volume sliders (Expressive's tall ones),
// the tiles (small: the icon; large: icon button, name and state), the media
// player, and a footer with the battery, Edit controls, Settings and Power.
// A tile's page (Wi-Fi, Bluetooth, Sound…) opens in the panel itself, sliding
// in from the side with a back arrow, as Android's do. Edit controls is
// Android's editor in place: click a tile to change its size, drag to move
// it, − to remove it, and the controls to add underneath.
Popover {
    id: qs

    popoverName: "controlCenter"
    targetX: (screen?.width ?? 1920) - implicitWidth
    radius: Theme.radius.panel
    padding: 16
    contentWidth: 368

    readonly property int gap: 8
    readonly property int tileHeight: 72
    readonly property real unit: (contentWidth - 3 * gap) / 4
    // How tall the panel may get: under the menu bar, a margin from the bottom
    readonly property real maxHeight: (screen?.height ?? 1080) - 34 - 2 * padding - 16

    // ── The open page, kept in step with Panels.ccPage both ways ────────
    // (without a binding that the first local change would break)
    property string page: Panels.ccPage
    Connections {
        target: Panels
        function onCcPageChanged() {
            if (Panels.screen === qs.screen)
                qs.page = Panels.ccPage;
        }
    }
    onPageChanged: {
        if (paging)
            shownPage = page;
        if (Panels.screen === qs.screen && Panels.ccPage !== page)
            Panels.ccPage = page;
    }
    // A page this panel has (below) is open; the page drawn stays while it
    // slides out
    readonly property bool paging: page !== "" && pageComponents[page] !== undefined
    property string shownPage: paging ? page : ""
    property real pageT: paging ? 1 : 0
    Behavior on pageT {
        SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.002 }
    }
    onPageTChanged: if (!paging && pageT < 0.02) shownPage = ""

    readonly property bool editing: Panels.ccEditing
    // Escape (or the back arrow) leaves a page or the editor first
    sideOpen: page !== "" || editing
    onCloseSide: {
        if (page !== "")
            page = "";
        else
            Panels.ccEditing = false;
    }
    // Opening: the pieces rise in one after another (QSRise). Only the
    // screen that closed resets the shared state.
    property real stagger: 0
    NumberAnimation {
        id: staggerAnim
        target: qs
        property: "stagger"
        from: 0
        to: 1
        duration: 360
    }
    onShownChanged: {
        if (shown) {
            stagger = 0;
            staggerAnim.restart();
        } else if (Panels.screen === qs.screen) {
            Panels.ccEditing = false;
            page = "";
        }
    }
    Component.onCompleted: if (shown) staggerAnim.restart()

    ModuleDefs { id: moduleDefs }
    PointerMenu { id: choices }

    // A picture for the avatar, if there is one
    property string avatarPath: ""
    Process {
        running: true
        command: ["sh", "-c", "for f in \"$HOME/.face\" \"$HOME/.face.icon\" \"/var/lib/AccountsService/icons/$USER\"; do [ -s \"$f\" ] && [ -r \"$f\" ] && { echo \"$f\"; exit 0; }; done; true"]
        stdout: StdioCollector {
            onStreamFinished: qs.avatarPath = text.trim()
        }
    }

    // ── The layout, Android's way ───────────────────────────────────────
    // The same saved layout as Control Center's: sliders go to the top, the
    // media player under the tiles, and every other module is a tile, large
    // when it's two columns wide there (2×1, 2×2), small at 1×1
    function form(e) {
        const k = moduleDefs.all[e.id]?.kind;
        return k === "slider" ? "slider" : k === "media" ? "media" : e.w >= 2 ? "large" : "small";
    }
    readonly property var sliderIds: moduleDefs.layout.filter(e => form(e) === "slider").map(e => e.id)
    readonly property bool hasMedia: moduleDefs.layout.some(e => form(e) === "media")
    readonly property var tilePack: moduleDefs.pack(moduleDefs.layout.filter(e => form(e) === "small" || form(e) === "large").map(e => ({ id: e.id, w: form(e) === "large" ? 2 : 1, h: 1 })), 4)
    // (the tiles' model is their ids, so a tile's state changing doesn't
    // rebuild them; each looks up its place)
    readonly property var tileIds: tilePack.map(p => p.id)
    readonly property var tilePos: {
        const m = {};
        for (const p of tilePack)
            m[p.id] = p;
        return m;
    }
    readonly property int tileRows: tilePack.reduce((m, p) => Math.max(m, p.row + 1), 0)

    // Edit controls: a click switches a tile between small and large (the
    // next size it offers that's drawn differently here)
    function resize(id) {
        const e = moduleDefs.layout.find(x => x.id === id);
        if (!e)
            return;
        const sizes = moduleDefs.all[id].sizes;
        const now = form(e);
        const i = sizes.indexOf(e.w + "x" + e.h);
        for (let n = 1; n < sizes.length; n++) {
            const s = sizes[(i + n + sizes.length) % sizes.length];
            const [w, h] = s.split("x").map(Number);
            if (form({ id: id, w: w, h: h }) !== now) {
                moduleDefs.setSize(id, s);
                return;
            }
        }
    }
    // A dragged tile lands in the place of the tile nearest its centre
    function dropAt(id, cx, cy) {
        let best = "", bestD = 1e9;
        for (const p of tilePack) {
            const ex = p.col * (unit + gap) + (p.w * unit + (p.w - 1) * gap) / 2;
            const ey = p.row * (tileHeight + gap) + tileHeight / 2;
            const d = (ex - cx) * (ex - cx) + (ey - cy) * (ey - cy);
            if (d < bestD) {
                bestD = d;
                best = p.id;
            }
        }
        if (best !== "" && best !== id)
            moduleDefs.moveModule(id, moduleDefs.layout.findIndex(e => e.id === best));
    }

    // ── The panel ───────────────────────────────────────────────────────
    Item {
        id: body
        width: qs.contentWidth
        readonly property real target: Math.min(qs.maxHeight, qs.paging ? pageLoader.height : home.height)
        height: target
        Behavior on height {
            SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.5 }
        }
        // (clipped only while it changes height or pages slide, so badges
        // and focus rings at the edges show at rest)
        clip: Math.abs(height - target) > 0.5 || (qs.pageT > 0.01 && qs.pageT < 0.99) || home.height > qs.maxHeight

        // Home: sliders, tiles, media, footer (or the editor)
        Column {
            id: home
            width: parent.width
            spacing: 12
            visible: qs.pageT < 0.99
            enabled: !qs.paging
            opacity: 1 - Math.max(0, Math.min(1, qs.pageT))
            x: -40 * qs.pageT

            // Edit controls: back (Done), the title, Reset, and how it works
            Loader {
                id: editHeader
                active: qs.editing
                visible: active
                width: parent.width
                sourceComponent: Column {
                    spacing: 4
                    Item {
                        width: parent.width
                        height: 48
                        IconButton {
                            id: doneButton
                            anchors.verticalCenter: parent.verticalCenter
                            width: 40
                            height: 40
                            icon: "caret-left"
                            iconSize: 22
                            iconColor: Theme.m3.onSurface
                            tooltip: "Done"
                            onClicked: Panels.ccEditing = false
                        }
                        Text {
                            anchors { left: doneButton.right; leftMargin: 8; right: resetButton.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            text: "Edit controls"
                            elide: Text.ElideRight
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 22 }
                        }
                        TextButton {
                            id: resetButton
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            text: "Reset layout"
                            onClicked: Config.options.controlCenter.layout = []
                        }
                    }
                    Text {
                        width: parent.width
                        leftPadding: 4
                        rightPadding: 4
                        bottomPadding: 4
                        wrapMode: Text.Wrap
                        text: "Click a control to change its size, drag it to move it, or use − to remove it."
                        color: Theme.m3.onSurfaceVariant
                        font { family: Theme.font.family; pixelSize: 13 }
                    }
                }
            }

            // Brightness, volume (and the microphone, if added): Expressive's
            // tall sliders; those with a page get a button for it
            Column {
                id: sliders
                width: parent.width
                spacing: 8
                visible: qs.sliderIds.length > 0
                Repeater {
                    model: ScriptModel { values: qs.sliderIds }
                    Item {
                        id: sl
                        required property string modelData
                        required property int index
                        readonly property var def: moduleDefs.all[modelData] ?? ({})
                        width: sliders.width
                        height: qs.editing ? 56 : 60
                        QSRise {
                            id: slRise
                            order: sl.index
                            stagger: qs.stagger
                        }
                        opacity: Math.min(1, slRise.value)
                        transform: Translate { y: 12 * (1 - slRise.value) }

                        QSSlider {
                            visible: !qs.editing
                            anchors { left: parent.left; right: slMore.visible ? slMore.left : parent.right; rightMargin: slMore.visible ? 8 : 0; verticalCenter: parent.verticalCenter }
                            trackHeight: 52
                            icon: sl.def.icon ?? ""
                            accessibleName: sl.def.label ?? sl.modelData
                            value: sl.modelData === "display" ? Brightness.value : sl.modelData === "micLevel" ? (Audio.micMuted ? 0 : Audio.micVolume) : (Audio.muted ? 0 : Audio.volume)
                            hasMore: !!sl.def.page
                            onMoved: v => {
                                if (sl.modelData === "display")
                                    Brightness.set(v, true);
                                else if (sl.modelData === "micLevel")
                                    Audio.setMicVolume(v, true);
                                else
                                    Audio.setVolume(v, true);
                            }
                            onMore: qs.page = sl.def.page
                        }
                        // Its page (Sound: outputs and apps; Microphone: inputs)
                        TonalButton {
                            id: slMore
                            visible: !qs.editing && !!sl.def.page
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            width: 52
                            height: 52
                            corner: 16
                            icon: "sliders-horizontal"
                            tip: (sl.def.label ?? "") + " options"
                            onClicked: qs.page = sl.def.page
                        }
                        EditRow {
                            visible: qs.editing
                            anchors.fill: parent
                            icon: sl.def.icon ?? ""
                            label: sl.def.label ?? sl.modelData
                            onRemoveRequested: Qt.callLater(moduleDefs.removeModule, sl.modelData)
                        }
                    }
                }
            }

            // The tiles: four columns, small ones one wide, large ones two
            Item {
                id: grid
                width: parent.width
                visible: qs.tileIds.length > 0
                height: qs.tileRows * qs.tileHeight + Math.max(0, qs.tileRows - 1) * qs.gap
                Behavior on height {
                    SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.5 }
                }
                Repeater {
                    model: ScriptModel { values: qs.tileIds }
                    QSTile {
                        id: tile
                        required property string modelData
                        required property int index
                        readonly property var pos: qs.tilePos[modelData] ?? ({ col: 0, row: 0, w: 1 })
                        moduleId: modelData
                        defs: moduleDefs
                        large: pos.w >= 2
                        editing: qs.editing
                        width: pos.w * qs.unit + (pos.w - 1) * qs.gap
                        height: qs.tileHeight
                        Behavior on width {
                            SpringAnimation { spring: Theme.motion.springDefault.spring; damping: Theme.motion.springDefault.damping; epsilon: 0.3 }
                        }
                        // Its place, gliding there on the default spring when
                        // the layout changes; under the pointer while dragged
                        readonly property real homeX: pos.col * (qs.unit + qs.gap)
                        readonly property real homeY: pos.row * (qs.tileHeight + qs.gap)
                        property vector2d lastT: Qt.vector2d(0, 0)
                        function settle() {
                            xAnim.to = homeX;
                            yAnim.to = homeY;
                            xAnim.restart();
                            yAnim.restart();
                        }
                        Component.onCompleted: {
                            xAnim.stop();
                            yAnim.stop();
                            x = homeX;
                            y = homeY;
                        }
                        onHomeXChanged: if (!drag.active) settle()
                        onHomeYChanged: if (!drag.active) settle()
                        SpringAnimation {
                            id: xAnim
                            target: tile
                            property: "x"
                            spring: Theme.motion.springDefault.spring
                            damping: Theme.motion.springDefault.damping
                            epsilon: 0.3
                        }
                        SpringAnimation {
                            id: yAnim
                            target: tile
                            property: "y"
                            spring: Theme.motion.springDefault.spring
                            damping: Theme.motion.springDefault.damping
                            epsilon: 0.3
                        }
                        z: drag.active ? 5 : 0
                        QSRise {
                            id: tileRise
                            order: qs.sliderIds.length + tile.index
                            stagger: qs.stagger
                        }
                        scale: (0.86 + 0.14 * tileRise.value) * (drag.active ? 1.05 : 1)
                        opacity: Math.min(1, tileRise.value)
                        onOpenPage: p => qs.page = p
                        onMenuRequested: items => choices.open(items)
                        // (layout changes wait for the click or drag that asked to
                        // finish: they can remove the very tile handling it)
                        onRemoveRequested: Qt.callLater(moduleDefs.removeModule, modelData)
                        onResizeRequested: qs.resize(modelData)

                        DragHandler {
                            id: drag
                            target: null
                            enabled: qs.editing
                            onActiveTranslationChanged: {
                                if (!active)
                                    return;
                                tile.lastT = activeTranslation;
                                xAnim.stop();
                                yAnim.stop();
                                tile.x = tile.homeX + activeTranslation.x;
                                tile.y = tile.homeY + activeTranslation.y;
                            }
                            onActiveChanged: {
                                if (active) {
                                    tile.lastT = Qt.vector2d(0, 0);
                                    return;
                                }
                                Qt.callLater(qs.dropAt, tile.modelData, tile.homeX + tile.lastT.x + tile.width / 2, tile.homeY + tile.lastT.y + tile.height / 2);
                                tile.settle();
                            }
                        }
                    }
                }
            }

            // The media player
            Loader {
                id: media
                active: qs.hasMedia
                visible: active
                width: parent.width
                sourceComponent: qs.editing ? mediaEditRow : mediaCard
            }

            // Edit controls: everything that isn't here yet (then the ones
            // this computer can't use, greyed, with why)
            Loader {
                id: gallery
                active: qs.editing
                visible: active
                width: parent.width
                sourceComponent: Column {
                    spacing: 8
                    Text {
                        leftPadding: 4
                        topPadding: 4
                        text: "Add controls"
                        color: Theme.m3.primary
                        font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
                    }
                    Flickable {
                        Momentum {}
                        width: parent.width
                        // (what's left of the panel's height)
                        height: Math.min(contentHeight, Math.max(176, qs.maxHeight - editHeader.height - sliders.height - grid.height - (media.visible ? media.height : 0) - 4 * home.spacing - 40))
                        contentHeight: galleryList.implicitHeight
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        Column {
                            id: galleryList
                            width: parent.width
                            spacing: 2
                            Repeater {
                                model: ScriptModel {
                                    values: {
                                        const ids = Object.keys(moduleDefs.all).filter(id => !moduleDefs.layout.some(e => e.id === id));
                                        return ids.filter(id => moduleDefs.usable(id)).concat(ids.filter(id => !moduleDefs.usable(id)));
                                    }
                                }
                                ListRow {
                                    required property string modelData
                                    readonly property var def: moduleDefs.all[modelData]
                                    readonly property string whyNot: moduleDefs.whyNot(modelData)
                                    icon: def.icon
                                    title: def.label
                                    status: whyNot
                                    trailingIcon: whyNot === "" ? "plus" : ""
                                    enabled: whyNot === ""
                                    opacity: enabled ? 1 : 0.5
                                    onClicked: Qt.callLater(moduleDefs.addModule, modelData)
                                }
                            }
                        }
                    }
                }
            }

            // Footer: you (Android's user avatar) and the battery, then Edit
            // controls, Settings and Power
            Item {
                id: footer
                visible: !qs.editing
                width: parent.width
                height: 48
                QSRise {
                    id: footerRise
                    order: qs.sliderIds.length + qs.tileIds.length + 1
                    stagger: qs.stagger
                }
                opacity: Math.min(1, footerRise.value)
                transform: Translate { y: 12 * (1 - footerRise.value) }

                // Your picture (~/.face, or AccountsService's) in a cookie
                Item {
                    id: avatar
                    anchors.verticalCenter: parent.verticalCenter
                    width: 40
                    height: 40
                    Accessible.role: Accessible.StaticText
                    Accessible.name: Quickshell.env("USER") ?? ""
                    Image {
                        id: avatarImage
                        anchors.fill: parent
                        source: qs.avatarPath !== "" ? "file://" + qs.avatarPath : ""
                        sourceSize: Qt.size(80, 80)
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }
                    MaterialShape {
                        anchors.fill: parent
                        shape: "cookie9"
                        color: Theme.m3.tertiaryContainer
                        source: avatarImage.status === Image.Ready && avatarImage.source != "" ? avatarImage : null
                    }
                    Icon {
                        anchors.centerIn: parent
                        visible: avatarImage.status !== Image.Ready || avatarImage.source == ""
                        name: "user"
                        fill: true
                        size: 18
                        color: Theme.m3.onTertiaryContainer
                    }
                }
                Rectangle {
                    visible: Island.hasBattery
                    anchors { left: avatar.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
                    width: batteryRow.implicitWidth + 28
                    height: 40
                    radius: 20
                    color: Theme.m3.surfaceContainerHigh
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Battery " + Math.round(Island.level * 100) + "%" + (Island.charging ? ", charging" : "")
                    Row {
                        id: batteryRow
                        anchors.centerIn: parent
                        spacing: 6
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: Island.charging ? "battery-charging" : Island.level < 0.2 ? "battery-low" : "battery-high"
                            fill: true
                            size: 18
                            color: Island.level < 0.2 && !Island.charging ? Theme.m3.error : Theme.m3.onSurfaceVariant
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: Math.round(Island.level * 100) + "%"
                            color: Theme.m3.onSurface
                            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium; features: { "tnum": 1 } }
                        }
                    }
                }
                Row {
                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                    spacing: 8
                    TonalButton {
                        icon: "pencil-simple"
                        tip: "Edit controls"
                        onClicked: Panels.ccEditing = true
                    }
                    TonalButton {
                        icon: "gear-six"
                        tip: "Open Settings"
                        onClicked: Panels.openSettings()
                    }
                    TonalButton {
                        width: 56
                        strong: true
                        icon: "power"
                        tip: "Power options"
                        onClicked: Panels.show("session", qs.screen)
                    }
                }
            }
        }

        // A tile's page, sliding in from the side
        Loader {
            id: pageLoader
            width: parent.width
            active: qs.shownPage !== ""
            visible: active && qs.pageT > 0.01
            opacity: Math.max(0, Math.min(1, qs.pageT))
            x: 40 * (1 - qs.pageT)
            sourceComponent: qs.pageComponents[qs.shownPage] ?? null
        }
    }

    // ── Pages ───────────────────────────────────────────────────────────
    // Control Center's own, with Material's header (DetailPage); back
    // returns to the tiles
    readonly property var pageComponents: ({
            wifi: wifiPage,
            bluetooth: bluetoothPage,
            sound: soundPage,
            mic: micPage,
            focus: focusPage,
            caffeine: caffeinePage,
            resources: resourcesPage,
            sharing: sharingPage,
            mirroring: mirroringPage,
            cast: castPage
        })
    Component { id: wifiPage; WifiPage { onBack: qs.page = "" } }
    Component { id: bluetoothPage; BluetoothPage { onBack: qs.page = "" } }
    Component { id: soundPage; SoundPage { onBack: qs.page = "" } }
    Component { id: micPage; MicPage { onBack: qs.page = "" } }
    Component { id: focusPage; FocusPage { onBack: qs.page = "" } }
    Component { id: caffeinePage; CaffeinePage { onBack: qs.page = "" } }
    Component {
        id: resourcesPage
        ResourcesPage {
            onBack: qs.page = ""
            networkExtra: Component { SpeedTestCard { compact: true; settingsLink: true } }
        }
    }
    Component { id: sharingPage; SharingPage { onBack: qs.page = "" } }
    Component { id: mirroringPage; MirroringPage { onBack: qs.page = "" } }
    Component { id: castPage; CastPage { onBack: qs.page = "" } }

    // The media card, and its row in Edit controls
    Component {
        id: mediaCard
        QSMedia {
            id: card
            shown: qs.shown
            QSRise {
                id: mediaRise
                order: qs.sliderIds.length + qs.tileIds.length
                stagger: qs.stagger
            }
            opacity: Math.min(1, mediaRise.value)
            transform: Translate { y: 12 * (1 - mediaRise.value) }
        }
    }
    Component {
        id: mediaEditRow
        EditRow {
            height: 56
            icon: "music-notes"
            label: moduleDefs.all.nowPlaying?.label ?? "Now Playing"
            onRemoveRequested: Qt.callLater(moduleDefs.removeModule, "nowPlaying")
        }
    }

    // ── Small parts ─────────────────────────────────────────────────────
    // A round tonal icon button (strong: in primaryContainer, a wider pill,
    // as Android's power button); a press squares it off
    component TonalButton: Rectangle {
        id: tb
        property string icon
        property string tip
        property bool strong: false
        property real corner: height / 2
        signal clicked
        width: 40
        height: 40
        readonly property color ink: strong ? Theme.m3.onPrimaryContainer : Theme.m3.onSurfaceVariant
        property real shownCorner: tbMouse.pressed ? Math.min(12, corner) : corner
        Behavior on shownCorner {
            SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.2 }
        }
        radius: shownCorner
        color: Qt.tint(strong ? Theme.m3.primaryContainer : Theme.m3.surfaceContainerHighest, Qt.alpha(ink, tbMouse.pressed ? 0.1 : tbMouse.containsMouse ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        activeFocusOnTab: visible
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                tb.clicked();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: tip
        Accessible.onPressAction: tb.clicked()
        FocusRing {
            radius: tb.radius + 5
        }
        Icon {
            anchors.centerIn: parent
            name: tb.icon
            fill: tb.strong
            size: 20
            color: tb.ink
        }
        MouseArea {
            id: tbMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: tb.clicked()
        }
    }

    // A text button (Reset layout): primary text, a state layer under it
    component TextButton: Rectangle {
        id: txb
        property string text
        signal clicked
        width: txbLabel.implicitWidth + 24
        height: 36
        radius: height / 2
        color: Qt.alpha(Theme.m3.primary, txbMouse.pressed ? 0.1 : txbMouse.containsMouse ? 0.08 : 0)
        activeFocusOnTab: visible
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                txb.clicked();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: txb.clicked()
        FocusRing {
            radius: txb.radius + 5
        }
        Text {
            id: txbLabel
            anchors.centerIn: parent
            text: txb.text
            color: Theme.m3.primary
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
        }
        MouseArea {
            id: txbMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: txb.clicked()
        }
    }

    // A slider or the media player in Edit controls: a row with its icon
    // and name, and − to remove it
    component EditRow: Item {
        id: er
        property string icon
        property string label
        signal removeRequested
        Rectangle {
            anchors.fill: parent
            radius: 20
            color: Theme.m3.surfaceContainerHighest
        }
        Icon {
            id: erIcon
            x: 20
            anchors.verticalCenter: parent.verticalCenter
            name: er.icon
            size: 20
            color: Theme.m3.onSurface
        }
        Text {
            anchors { left: erIcon.right; leftMargin: 16; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
            text: er.label
            elide: Text.ElideRight
            color: Theme.m3.onSurface
            font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
        }
        Rectangle {
            x: -6
            y: -6
            z: 10
            width: 24
            height: 24
            radius: 12
            color: Theme.m3.inverseSurface
            activeFocusOnTab: true
            Keys.onPressed: event => {
                if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Delete) && !event.isAutoRepeat) {
                    er.removeRequested();
                    event.accepted = true;
                }
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Remove " + er.label
            FocusRing {
                radius: 17
            }
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
                onClicked: er.removeRequested()
            }
        }
    }
}
