import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.services

// The welcome screen, once, on a new setup (Config welcome.done; Settings →
// About shows it again, so does `ipc call welcome show`): over your
// wallpaper, blurred, a card that asks three things, one a step:
//   1 the look: Glass or Material (experimental; only where its files are)
//   2 light, dark, or auto (light from sunrise to sunset)
//   3 a wallpaper: the bundled ones, or a picture of your own
// then "Start using iRice". Light or dark and the wallpaper apply as you
// pick them; the look last, as Settings → Appearance switches it (the shell
// restarts behind the switching screen; apps keep running). "Set up later"
// on every step puts it away for good. Enter continues, Esc goes back, Tab
// moves between the choices (Space or Enter picks one).
Scope {
    id: root

    property bool shown: false // asked for
    property bool alive: false // on screen, through its exit
    property var where: null // the display it opened on
    property int step: 0
    readonly property int steps: 4
    property string look: Config.options.appearance.look

    readonly property bool still: Config.options.appearance.reduceMotion === true || CoolDown.motionScale <= 0
    readonly property color ink: Theme.color.label
    readonly property color ink2: Theme.color.secondary
    readonly property color accent: Theme.material ? Theme.m3.primary : Theme.accent
    readonly property color onAccent: Theme.material ? Theme.m3.onPrimary : Theme.color.onAccent

    function open() {
        if (alive && shown)
            return;
        look = Config.options.appearance.look;
        step = 0;
        where = Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0];
        shown = true;
        alive = true;
    }
    function later() {
        Config.options.welcome.done = true;
        shown = false;
    }
    function finish() {
        Config.options.welcome.done = true;
        shown = false;
        if (look !== Config.options.appearance.look && (look === "glass" || materialAvailable)) {
            Config.options.appearance.look = look;
            Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/switch-look.sh", look]);
        }
    }
    function next() {
        if (step < steps - 1)
            step++;
        else
            finish();
    }
    function back() {
        if (step > 0)
            step--;
    }

    // Once, on a new setup: when the desktop is up and unlocked
    Timer {
        running: Config.ready && !Config.options.welcome.done && !System.locked && !root.alive
        interval: 2500
        onTriggered: root.open()
    }
    IpcHandler {
        target: "welcome"
        function show(): void { root.open(); }
    }
    Connections {
        target: Panels
        function onWelcomeRequested() {
            Panels.settingsOpen = false; // (it opens over everything; Settings would wait behind it)
            root.open();
        }
    }

    // Where the bundled pictures really are (the Material look runs from a
    // folder of links: the settings keep the files' own paths), and whether
    // the Material look is there (material/, beside the shell)
    property string tahoeDir: ""
    property string materialDir: ""
    readonly property bool materialAvailable: Theme.material === true || materialDir !== ""
    Process {
        running: true
        command: ["sh", "-c", "s=\"$1\"; r=$(dirname \"$(readlink -f \"$s\")\"); for d in \"$s/assets/wallpapers\" \"$r/shell/assets/wallpapers\"; do [ -f \"$d/tahoe-day.jpg\" ] && { printf 'tahoe\\t%s\\n' \"$(dirname \"$(readlink -f \"$d/tahoe-day.jpg\")\")\"; break; }; done; for d in \"$s/assets/wallpapers\" \"$r/material/assets/wallpapers\"; do [ -f \"$d/material-light.jpg\" ] && { printf 'material\\t%s\\n' \"$(dirname \"$(readlink -f \"$d/material-light.jpg\")\")\"; break; }; done; true", "sh", Quickshell.shellDir]
        stdout: StdioCollector {
            onStreamFinished: {
                for (const l of text.split("\n")) {
                    const [k, v] = l.split("\t");
                    if (k === "tahoe")
                        root.tahoeDir = v;
                    else if (k === "material")
                        root.materialDir = v;
                }
            }
        }
    }

    // The wallpapers offered: the Tahoe set through the day, three of its
    // pictures, and Material's light and dark pair (where it's there)
    readonly property var walls: {
        const w = [];
        w.push({ id: "time", name: "Through the day", preview: tahoeDir ? tahoeDir + "/tahoe-dawn.jpg" : "" });
        for (const n of ["day", "dusk", "night"])
            w.push({ id: n, name: "Tahoe " + n[0].toUpperCase() + n.slice(1), preview: tahoeDir ? tahoeDir + "/tahoe-" + n + ".jpg" : "", path: tahoeDir ? tahoeDir + "/tahoe-" + n + ".jpg" : "" });
        if (materialDir)
            w.push({ id: "material", name: "Material, light and dark", preview: materialDir + (Theme.dark ? "/material-dark.jpg" : "/material-light.jpg"), light: materialDir + "/material-light.jpg", dark: materialDir + "/material-dark.jpg" });
        return w;
    }
    function useWall(w) {
        if (w.id === "time") {
            Config.options.wallpaper.live = "";
            Config.options.wallpaper.dynamicFolder = "";
            Config.options.wallpaper.dynamic = "time";
        } else if (w.id === "material") {
            Wallpaper.useSet({ kind: "pair", light: w.light, dark: w.dark });
        } else if (w.path) {
            Wallpaper.set(w.path);
        }
    }
    function isWall(w) {
        const c = Config.options.wallpaper;
        if (w.id === "time")
            return c.dynamic === "time" && c.dynamicFolder === "";
        if (w.id === "material")
            return c.dynamic === "appearance" && c.lightPath === w.light && c.darkPath === w.dark;
        return c.dynamic === "off" && c.live === "" && (c.path === w.path || (w.id === "dusk" && c.path === ""));
    }
    // A picture of your own (anything the list doesn't have)
    readonly property bool ownWall: {
        const c = Config.options.wallpaper;
        return !walls.some(w => isWall(w)) && (c.dynamic !== "off" || c.path !== "" || c.live !== "");
    }

    LazyLoader {
        active: root.alive

        PanelWindow {
            id: win
            screen: root.where
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "irice-welcome"
            WlrLayershell.keyboardFocus: root.shown ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            color: "transparent"

            // In (fade and rise, ease-out) and out (a third faster, ease-in)
            property real t: 0
            Component.onCompleted: t = 1
            Behavior on t {
                NumberAnimation { duration: root.still ? 0 : root.shown ? 300 : 200; easing.type: root.shown ? Easing.OutCubic : Easing.InCubic }
            }
            Connections {
                target: root
                function onShownChanged() {
                    win.t = root.shown ? 1 : 0;
                }
            }
            onTChanged: if (t === 0 && !root.shown) root.alive = false

            // ── The wallpaper, blurred and dimmed ─────────────────────────
            Item {
                anchors.fill: parent
                opacity: win.t
                Image {
                    id: backdrop
                    anchors.fill: parent
                    source: Wallpaper.source
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(Math.round(win.width / 4), Math.round(win.height / 4))
                    asynchronous: true
                    visible: false
                }
                MultiEffect {
                    anchors.fill: parent
                    source: backdrop
                    blurEnabled: true
                    blur: 1
                    blurMax: 48
                }
                Rectangle {
                    anchors.fill: parent
                    color: Theme.dark ? Qt.rgba(0, 0, 0, 0.45) : Qt.rgba(1, 1, 1, 0.3)
                }
                // Clicks around the card do nothing
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.AllButtons
                    onWheel: wheel => wheel.accepted = true
                }
            }

            // ── The card ──────────────────────────────────────────────────
            Rectangle {
                id: card
                width: Math.min(760, win.width - 64)
                height: column.implicitHeight + 64
                x: Math.round((win.width - width) / 2)
                y: Math.round((win.height - height) / 2) + (1 - win.t) * 12
                opacity: win.t
                radius: Theme.material ? 28 : Theme.radius.panel
                color: Theme.material ? Theme.m3.surfaceContainer : Theme.dark ? "#1E1E21" : "#F5F5F7"
                border.width: Theme.material ? 0 : 1
                border.color: Theme.color.separator

                focus: true
                Keys.onReturnPressed: root.next()
                Keys.onEnterPressed: root.next()
                Keys.onEscapePressed: root.back()

                Column {
                    id: column
                    x: 32
                    y: 32
                    width: card.width - 64
                    spacing: 32

                    // Where you are, the title and one line of help
                    Column {
                        width: parent.width
                        spacing: 8
                        Text {
                            text: "Step " + (root.step + 1) + " of " + root.steps
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: 13; weight: Font.Medium; features: { "tnum": 1 } }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: ["Welcome to iRice", "Light or dark?", "Choose a wallpaper", "You're all set"][root.step]
                            color: root.ink
                            font { family: Theme.font.family; pixelSize: 24; weight: Font.DemiBold; letterSpacing: -0.2 }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: [
                                "Pick a look for your desktop. You can change it any time in Settings → Appearance.",
                                "Auto is light from sunrise to sunset and dark at night.",
                                "Pick one of these, or a picture of your own.",
                                root.look !== Config.options.appearance.look ? (root.look === "material" ? "iRice switches to Material when you start. Your apps keep running." : "iRice switches to Glass when you start. Your apps keep running.") : "Everything you picked can be changed in Settings."
                            ][root.step]
                            color: root.ink2
                            font { family: Theme.font.family; pixelSize: 14 }
                        }
                    }

                    // The step's choices, sliding in from the side they come from
                    Item {
                        id: stage
                        width: parent.width
                        height: Math.max(1, [lookPage, modePage, wallPage, donePage][root.step].implicitHeight)
                        property int shownStep: root.step
                        property real slide: 0
                        property real fade: 1
                        Connections {
                            target: root
                            function onStepChanged() {
                                const dir = root.step > stage.shownStep ? 1 : -1;
                                stage.shownStep = root.step;
                                if (root.still)
                                    return;
                                stage.slide = 24 * dir;
                                stage.fade = 0;
                                enter.restart();
                            }
                        }
                        ParallelAnimation {
                            id: enter
                            NumberAnimation { target: stage; property: "slide"; to: 0; duration: 250; easing.type: Easing.OutCubic }
                            NumberAnimation { target: stage; property: "fade"; to: 1; duration: 200; easing.type: Easing.OutCubic }
                        }

                        Item {
                            id: pages
                            width: parent.width
                            x: stage.slide
                            opacity: stage.fade

                            // 1 · the look
                            Row {
                                id: lookPage
                                visible: root.step === 0
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 24
                                Repeater {
                                    model: root.materialAvailable ? ["glass", "material"] : ["glass"]
                                    Column {
                                        id: lookCard
                                        required property string modelData
                                        spacing: 8
                                        topPadding: 8
                                        LookPreview {
                                            width: Math.min(320, (pages.width - 24) / 2)
                                            height: Math.round(width * 0.625)
                                            look: lookCard.modelData
                                            selected: root.look === lookCard.modelData
                                            onChosen: root.look = lookCard.modelData
                                            FocusRing { radius: 16 }
                                        }
                                        Text {
                                            anchors.horizontalCenter: parent.horizontalCenter
                                            text: lookCard.modelData === "material" ? "Material" : "Glass"
                                            color: root.ink
                                            font { family: Theme.font.family; pixelSize: 16; weight: root.look === lookCard.modelData ? Font.DemiBold : Font.Medium }
                                        }
                                        Text {
                                            anchors.horizontalCenter: parent.horizontalCenter
                                            text: lookCard.modelData === "material" ? "Material 3 Expressive · experimental" : "Liquid glass, like macOS Tahoe"
                                            color: root.ink2
                                            font { family: Theme.font.family; pixelSize: 13 }
                                        }
                                    }
                                }
                            }

                            // 2 · light, dark or auto
                            Row {
                                id: modePage
                                visible: root.step === 1
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 16
                                readonly property real tileW: Math.min(200, (pages.width - 32) / 3)
                                Repeater {
                                    model: [
                                        { id: "light", name: "Light" },
                                        { id: "dark", name: "Dark" },
                                        { id: "auto", name: "Auto" }
                                    ]
                                    ChoiceCard {
                                        id: modeCard
                                        required property var modelData
                                        width: modePage.tileW
                                        frameHeight: Math.round(width * 0.6)
                                        label: modelData.name
                                        selected: Config.options.appearance.mode === modelData.id
                                        onChosen: Config.options.appearance.mode = modelData.id
                                        // A window in that mode: light, dark, or one of each
                                        Row {
                                            anchors.fill: parent
                                            Rectangle {
                                                width: parent.width * (modeCard.modelData.id === "auto" ? 0.5 : 1)
                                                height: parent.height
                                                color: modeCard.modelData.id === "dark" ? "#1C1C1E" : "#F2F2F5"
                                            }
                                            Rectangle {
                                                visible: modeCard.modelData.id === "auto"
                                                width: parent.width * 0.5
                                                height: parent.height
                                                color: "#1C1C1E"
                                            }
                                        }
                                        Rectangle {
                                            anchors.centerIn: parent
                                            width: parent.width * 0.56
                                            height: parent.height * 0.5
                                            radius: 8
                                            color: modeCard.modelData.id === "dark" ? "#2C2C30" : "white"
                                            border.width: 1
                                            border.color: Qt.rgba(0, 0, 0, 0.12)
                                            Rectangle {
                                                x: 8
                                                y: 8
                                                width: parent.width * 0.5
                                                height: 6
                                                radius: 3
                                                color: root.accent
                                            }
                                            Rectangle {
                                                x: 8
                                                y: 20
                                                width: parent.width * 0.7
                                                height: 4
                                                radius: 2
                                                color: modeCard.modelData.id === "dark" ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(0, 0, 0, 0.2)
                                            }
                                        }
                                    }
                                }
                            }

                            // 3 · a wallpaper
                            Grid {
                                id: wallPage
                                visible: root.step === 2
                                anchors.horizontalCenter: parent.horizontalCenter
                                columns: 3
                                spacing: 16
                                readonly property real tileW: Math.floor((pages.width - 32) / 3)
                                Repeater {
                                    model: root.walls
                                    ChoiceCard {
                                        id: wallCard
                                        required property var modelData
                                        width: wallPage.tileW
                                        frameHeight: Math.round(width * 0.56)
                                        label: modelData.name
                                        selected: root.isWall(modelData)
                                        onChosen: root.useWall(modelData)
                                        Image {
                                            anchors.fill: parent
                                            source: wallCard.modelData.preview ? Wallpaper.fileUrl(wallCard.modelData.preview) : ""
                                            fillMode: Image.PreserveAspectCrop
                                            sourceSize: Qt.size(400, 224)
                                            asynchronous: true
                                        }
                                    }
                                }
                                // A picture of your own
                                ChoiceCard {
                                    width: wallPage.tileW
                                    frameHeight: Math.round(width * 0.56)
                                    label: root.ownWall ? "Your picture" : "Choose a picture…"
                                    selected: root.ownWall
                                    onChosen: FilePicker.pick({ title: "Choose a wallpaper", start: "~/Pictures", filter: "Images|*.jpg *.jpeg *.png *.webp" }, p => Wallpaper.set(p))
                                    Image {
                                        anchors.fill: parent
                                        visible: root.ownWall
                                        source: root.ownWall ? Wallpaper.source : ""
                                        fillMode: Image.PreserveAspectCrop
                                        sourceSize: Qt.size(400, 224)
                                        asynchronous: true
                                    }
                                    Icon {
                                        anchors.centerIn: parent
                                        visible: !root.ownWall
                                        name: "image"
                                        size: 24
                                        color: root.ink2
                                    }
                                }
                            }

                            // 4 · done: the look you picked, on your wallpaper
                            Item {
                                id: donePage
                                visible: root.step === 3
                                width: parent.width
                                implicitHeight: summary.height + 8
                                LookPreview {
                                    id: summary
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: 4
                                    width: Math.min(360, parent.width)
                                    height: Math.round(width * 0.625)
                                    look: root.look
                                    selected: false
                                    activeFocusOnTab: false
                                }
                            }
                        }
                    }

                    // Set up later · Back · Continue
                    Item {
                        width: parent.width
                        height: 44
                        WelcomeButton {
                            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                            visible: root.step < root.steps - 1
                            kind: "text"
                            text: "Set up later"
                            onClicked: root.later()
                        }
                        Row {
                            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                            spacing: 12
                            WelcomeButton {
                                visible: root.step > 0
                                kind: "secondary"
                                text: "Back"
                                onClicked: root.back()
                            }
                            WelcomeButton {
                                kind: "primary"
                                text: root.step === root.steps - 1 ? "Start using iRice" : "Continue"
                                onClicked: root.next()
                            }
                        }
                    }
                }
            }
        }
    }

    // A picture to choose (a mode, a wallpaper): its frame, the ring when
    // it's chosen, its name under it
    component ChoiceCard: Item {
        id: cc
        property string label
        property bool selected: false
        property real frameHeight: 112
        default property alias content: inner.data
        signal chosen
        implicitHeight: frameHeight + 8 + 24
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                cc.chosen();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.RadioButton
        Accessible.name: label
        Accessible.checked: selected
        Accessible.onPressAction: cc.chosen()

        Rectangle {
            id: frame
            width: parent.width
            height: cc.frameHeight
            radius: Theme.material ? 16 : 12
            color: Theme.color.fill
            clip: true
            Item {
                id: inner
                anchors.fill: parent
            }
            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                color: "transparent"
                border.width: 1
                border.color: Theme.color.separator
            }
        }
        Rectangle {
            anchors.fill: frame
            anchors.margins: -5
            radius: frame.radius + 5
            color: "transparent"
            border.width: 3
            border.color: root.accent
            opacity: cc.selected ? 1 : ccMouse.containsMouse ? 0.35 : 0
            Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
        }
        Text {
            anchors { horizontalCenter: parent.horizontalCenter; top: frame.bottom; topMargin: 8 }
            width: Math.min(implicitWidth, parent.width)
            elide: Text.ElideRight
            text: cc.label
            color: root.ink
            font { family: Theme.font.family; pixelSize: 14; weight: cc.selected ? Font.DemiBold : Font.Normal }
        }
        MouseArea {
            id: ccMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: cc.chosen()
        }
        FocusRing {
            radius: frame.radius + 4
        }
    }

    // 44 px buttons: primary (the accent), secondary (a fill), text
    component WelcomeButton: Rectangle {
        id: wb
        property string text
        property string kind: "primary"
        signal clicked
        implicitWidth: Math.max(88, label.implicitWidth + (kind === "text" ? 24 : 40))
        implicitHeight: 44
        radius: Theme.material ? height / 2 : Theme.radius.control + 2
        color: kind === "primary" ? (wbMouse.pressed ? Qt.darker(root.accent, 1.15) : wbMouse.containsMouse ? Qt.darker(root.accent, 1.07) : root.accent)
            : kind === "secondary" ? (wbMouse.pressed ? Theme.color.fillPressed : wbMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill)
            : (wbMouse.pressed ? Theme.color.fillPressed : wbMouse.containsMouse ? Theme.color.fill : "transparent")
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        scale: wbMouse.pressed ? 0.97 : 1
        Behavior on scale { NumberAnimation { duration: Theme.motion.press } }
        activeFocusOnTab: true
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                wb.clicked();
                event.accepted = true;
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: text
        Accessible.onPressAction: wb.clicked()
        Text {
            id: label
            anchors.centerIn: parent
            text: wb.text
            color: wb.kind === "primary" ? root.onAccent : wb.kind === "text" ? (Theme.material ? Theme.m3.primary : Theme.accentInk) : root.ink
            font { family: Theme.font.family; pixelSize: 14; weight: Font.DemiBold }
        }
        MouseArea {
            id: wbMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: wb.clicked()
        }
        FocusRing {
            radius: wb.radius + 4
        }
    }
}
