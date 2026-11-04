import QtQuick
import QtQuick as QQ
import QtQuick.Controls as QQC
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    // The look: a picture of the desktop in each. Choosing one restarts the
    // shell in that look behind the switching screen (switch-look.sh)
    function choose(look) {
        if (Config.options.appearance.look === look)
            return;
        Config.options.appearance.look = look;
        Quickshell.execDetached(["setsid", "-f", Quickshell.shellDir + "/assets/scripts/switch-look.sh", look]);
    }
    // Material: two large preview cards across the page, as Android's
    // Wallpaper & style shows its choices
    Loader {
        width: parent.width
        active: Theme.material
        visible: active
        sourceComponent: Component {
            Column {
                readonly property string label: "Look" // (a search for it lands here)
                spacing: 12
                QQ.Row {
                    id: cards
                    spacing: 24
                    topPadding: 6
                    Repeater {
                        model: ["glass", "material"]
                        Column {
                            id: card
                            required property string modelData
                            readonly property bool chosen: Config.options.appearance.look === modelData
                            spacing: 12
                            LookPreview {
                                x: 6
                                width: (page.width - 24) / 2 - 12
                                height: width * 10 / 16
                                look: card.modelData
                                selected: card.chosen
                                onChosen: page.choose(card.modelData)
                            }
                            QQ.Row {
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 6
                                Icon {
                                    visible: card.chosen
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: "check-circle"
                                    fill: true
                                    size: 20
                                    color: Theme.m3.primary
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: card.modelData === "material" ? "Material" : "Glass"
                                    color: card.chosen ? Theme.m3.primary : Theme.m3.onSurface
                                    font { family: Theme.font.family; pixelSize: 16; weight: card.chosen ? Font.DemiBold : Font.Medium }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: page.choose(card.modelData)
                                    }
                                }
                            }
                        }
                    }
                }
                Text {
                    width: parent.width
                    leftPadding: 16
                    rightPadding: 16
                    wrapMode: Text.Wrap
                    text: "Material (experimental), after Google's Material 3 Expressive: tonal colour from your wallpaper, bold shapes, springy motion. Switching restarts the shell; apps keep running."
                    color: Theme.m3.onSurfaceVariant
                    font { family: Theme.font.family; pixelSize: 14 }
                }
            }
        }
    }

    Group {
        width: parent.width
        // Glass: small thumbnails in the first row, like macOS's appearance
        // choices (Material is experimental). Shown when the Material look's
        // files (material/, beside the shell) are there.
        Row {
            id: lookRow
            property bool available: false
            visible: !Theme.material && available
            label: "Look"
            detail: Config.options.appearance.look === "material" ? "Material (experimental), after Google's Material 3 Expressive. Switching restarts the shell; apps keep running." : "Glass. Material (experimental) follows Google's Material 3 Expressive; switching restarts the shell, apps keep running."
            Process {
                running: !Theme.material
                command: ["sh", "-c", "[ -f \"$(dirname \"$(readlink -f \"$1\")\")/material/shell.qml\" ]", "sh", Quickshell.shellDir]
                onExited: code => lookRow.available = code === 0
            }
            QQ.Row {
                spacing: 14
                Repeater {
                    model: ["glass", "material"]
                    Column {
                        id: thumb
                        required property string modelData
                        topPadding: 6
                        bottomPadding: 2
                        spacing: 6
                        LookPreview {
                            width: 128
                            height: 80
                            look: thumb.modelData
                            selected: Config.options.appearance.look === thumb.modelData
                            onChosen: page.choose(thumb.modelData)
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: thumb.modelData === "material" ? "Material" : "Glass"
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Config.options.appearance.look === thumb.modelData ? Font.DemiBold : Font.Normal }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: page.choose(thumb.modelData)
                            }
                        }
                    }
                }
            }
        }
        Row {
            label: "Appearance"
            detail: Config.options.appearance.mode === "auto" ? "Light from sunrise (" + Focus.fmt(Math.floor(Daylight.sunrise / 60) + ":" + Daylight.sunrise % 60) + ") to sunset (" + Focus.fmt(Math.floor(Daylight.sunset / 60) + ":" + Daylight.sunset % 60) + ")" : ""
            Segmented {
                width: 260
                readonly property var values: ["light", "dark", "auto"]
                options: ["Light", "Dark", "Auto"]
                current: Math.max(0, values.indexOf(Config.options.appearance.mode))
                onSelected: i => Config.options.appearance.mode = values[i]
            }
        }
        Row {
            label: "24-hour time"
            detail: "Clocks, events and alarms show " + (Config.options.appearance.clock24 ? "17:30" : "5:30 PM")
            Switch {
                checked: Config.options.appearance.clock24
                onToggled: Config.options.appearance.clock24 = !checked
            }
        }
        Row {
            label: "App icons"
            detail: ({ look: Theme.material ? "Your apps' own logos in your colours, like Android's themed icons" : "Your apps' own logos, frosted, like iOS's clear icons", themed: Theme.material ? "In your colours, like Android's themed icons" : "", dark: "Dark backgrounds, colourful artwork", clear: "Frosted and colourless", tinted: "Tinted with the accent colour", auto: "Dark icons in dark mode" })[Config.options.appearance.iconStyle] ?? ""
            Segmented {
                // (Themed: Material's, in its tonal colours)
                width: Theme.material ? 400 : 340
                readonly property var values: Theme.material ? ["default", "themed", "dark", "clear", "tinted", "auto"] : ["default", "dark", "clear", "tinted", "auto"]
                options: Theme.material ? ["Default", "Themed", "Dark", "Clear", "Tinted", "Auto"] : ["Default", "Dark", "Clear", "Tinted", "Auto"]
                current: Math.max(0, values.indexOf(Config.options.appearance.iconStyle === "look" ? (Theme.material ? "themed" : "clear") : Config.options.appearance.iconStyle)) // ("look": each look's own style)
                onSelected: i => Config.options.appearance.iconStyle = values[i]
            }
        }
        Row {
            visible: Theme.material
            label: "Colours from"
            detail: Config.options.appearance.materialSeed === "wallpaper" ? "Your wallpaper, like Material You" : "This colour"
            Flow {
                width: 9 * 22 + 8 * 8
                spacing: 8
                layoutDirection: Qt.RightToLeft
                Repeater {
                    // (seeds: Material's baseline purple and a hue wheel;
                    // none in Glass, which has no Wallpaper.seed)
                    model: !Theme.material ? [] : ["wallpaper", "#6750A4", "#B3261E", "#C26A00", "#8A7D00", "#386A20", "#006A6A", "#0061A4", "#7D5260"]
                    Rectangle {
                        required property string modelData
                        readonly property bool current: Config.options.appearance.materialSeed === modelData
                        width: 22
                        height: 22
                        radius: 11
                        color: modelData === "wallpaper" ? Wallpaper.seed : modelData
                        border.width: current ? 3 : 0
                        border.color: Theme.color.label
                        Icon {
                            anchors.centerIn: parent
                            visible: parent.modelData === "wallpaper"
                            name: "image"
                            fill: true
                            size: 11
                            color: "white"
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Config.options.appearance.materialSeed = parent.modelData
                        }
                    }
                }
            }
        }
        Row {
            visible: Theme.material
            label: "Colour style"
            detail: ({ tonalSpot: "Calm: one colour with soft companions", vibrant: "Saturated colours throughout", expressive: "Playful: contrasting companion colours", neutral: "Nearly grey, a hint of colour" })[Config.options.appearance.materialStyle] ?? ""
            Segmented {
                width: 340
                readonly property var values: ["tonalSpot", "vibrant", "expressive", "neutral"]
                options: ["Tonal", "Vibrant", "Expressive", "Neutral"]
                current: Math.max(0, values.indexOf(Config.options.appearance.materialStyle))
                onSelected: i => Config.options.appearance.materialStyle = values[i]
            }
        }
        Row {
            visible: Theme.material
            label: "Transparency"
            // (one control, its left end solid: Reduce transparency, a Glass
            // switch hidden here, shows as solid, and sliding up ends it)
            detail: Config.options.appearance.reduceTransparency ? "Solid: Reduce transparency is on. Slide to let the desktop show through." : Theme.material && Theme.transparency > 0 ? Math.round(Theme.transparency * 100) + "%: panels, menus, tiles and buttons show the blurred desktop behind them" : "Solid. Slide to let the blurred desktop show through panels, menus, tiles and buttons"
            Slider {
                width: 220
                from: 0
                to: 1
                value: Config.options.appearance.reduceTransparency ? 0 : Config.options.appearance.materialTransparency
                onMoved: v => {
                    Config.options.appearance.materialTransparency = Math.round(v * 20) / 20;
                    if (v > 0)
                        Config.options.appearance.reduceTransparency = false;
                }
            }
        }
        Row {
            visible: !Theme.material
            label: "Accent colour"
            detail: Config.options.appearance.accent === "wallpaper" ? "Following the wallpaper" : ""
            Flow {
                width: 9 * 22 + 8 * 8
                spacing: 8
                layoutDirection: Qt.RightToLeft
                Repeater {
                    model: ["wallpaper", "graphite", "green", "yellow", "orange", "red", "pink", "purple", "blue"]
                    Rectangle {
                        required property string modelData
                        readonly property bool current: Config.options.appearance.accent === modelData
                        width: 22
                        height: 22
                        radius: 11
                        color: modelData === "wallpaper" ? Wallpaper.accent : Theme.accents[modelData]
                        border.width: current ? 3 : 0
                        border.color: Theme.dark ? "white" : Qt.rgba(0, 0, 0, 0.5)
                        Icon {
                            anchors.centerIn: parent
                            visible: parent.modelData === "wallpaper"
                            name: "image"
                            fill: true
                            size: 11
                            color: "white"
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Config.options.appearance.accent = parent.modelData
                        }
                    }
                }
            }
        }
    }

    Group {
        width: parent.width
        SubpageLink {
            visible: !Theme.material // (Material surfaces are opaque: no glass to tune)
            icon: "drop-half"
            label: "Glass"
            detail: "Material, frost, lens and which apps match it"
            value: ({ liquid: "Glass", frosted: "Frost", solid: "Solid", custom: "Custom" })[Theme.glass.material] ?? ""
            page: "appearance/glass"
        }
        SubpageLink {
            visible: Theme.material // (the Glass page's Match apps, on its own)
            icon: "paint-brush"
            label: "Match apps"
            detail: "Colour GTK and Qt apps, terminals and browsers to match"
            page: "appearance/glass"
        }
    }

    Group {
        width: parent.width
        title: "Accessibility"
        Row {
            visible: !Theme.material // (Material surfaces are always solid)
            label: "Reduce transparency"
            detail: "Solid surfaces instead of glass, whatever the material above"
            Switch {
                checked: Config.options.appearance.reduceTransparency
                onToggled: Config.options.appearance.reduceTransparency = !checked
            }
        }
        Row {
            label: "Reduce motion"
            detail: "Fades instead of movement: island alerts, connecting a display (windows move without gliding) and the morning screen"
            Switch {
                checked: Config.options.appearance.reduceMotion
                onToggled: Config.options.appearance.reduceMotion = !checked
            }
        }
        Row {
            label: "Text contrast"
            detail: ["Light secondary text, as on macOS", "Easier-to-read text, icons and controls", "The clearest text, icons and controls"][Theme.contrastLevel]
            Segmented {
                width: 280
                readonly property var values: ["standard", "higher", "maximum"]
                options: ["Standard", "Higher", "Maximum"]
                current: Theme.contrastLevel
                onSelected: i => {
                    Config.options.appearance.contrast = values[i];
                    Config.options.appearance.increaseContrast = false; // (the old switch)
                }
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            Row {
                visible: Config.options.appearance.iconStyle === "clear" || Config.options.appearance.iconStyle === "tinted"
                label: "Icon contrast"
                detail: "How clearly each logo stands out from its tile; every logo is also evened out on its own"
                Slider {
                    width: 220
                    from: 0
                    to: 1
                    value: Config.options.appearance.iconContrast
                    onMoved: v => Config.options.appearance.iconContrast = Math.round(v * 100) / 100
                }
            }
            Row {
                label: "Emoji"
                detail: EmojiFont.available ? "In the emoji picker (Super+.) and Search" : EmojiFont.packageFor(EmojiFont.style) + " isn't installed; showing Noto until it is"
                Flow { // (Row here is the settings row)
                    spacing: 8
                    width: (EmojiFont.available ? 0 : emojiInstall.implicitWidth + 32) + 300
                    Squircle { // (Material: a 40 px tonal button)
                        visible: !EmojiFont.available
                        width: emojiInstall.implicitWidth + (Theme.material ? 32 : 24)
                        height: Theme.material ? 40 : 28
                        radius: Theme.material ? height / 2 : Theme.radius.control
                        color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, emojiInstallMouse.containsMouse ? 0.08 : 0)) : emojiInstallMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                        Text {
                            id: emojiInstall
                            anchors.centerIn: parent
                            text: "Install font"
                            color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium }
                        }
                        MouseArea {
                            id: emojiInstallMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: EmojiFont.install(EmojiFont.style)
                        }
                    }
                    Segmented {
                        width: 300
                        readonly property var values: ["android", "apple", "windows"]
                        options: ["Noto", "Apple", "Fluent"]
                        current: Math.max(0, values.indexOf(EmojiFont.style))
                        onSelected: i => Config.options.emoji.style = values[i]
                    }
                }
            }
        }
        Group {
            width: parent.width
            title: "Menu bar and shortcuts"
            footer: "The icon opens the system menu. Shortcut symbols are how keys are written in menus, Settings and the cheatsheet" + (Config.options.appearance.keySymbols === "auto" ? "; Auto follows the icon: Command uses ⌘ ⌃ ⌥ ⇧, others Super Ctrl Alt." : ".")
            Row {
                label: "Menu icon"
                Flow { // (Row here is the settings row)
                    spacing: 4
                    width: 8 * 40 + 7 * 4
                    Repeater {
                        model: [["tessera", "circle-wavy", "Badge"], ["distro", "", (SystemInfo.osName || "Your distro")], ["linux", "linux-logo", "Linux"], ["command", "command", "Command"], ["terminal", "terminal-window", "Terminal"], ["cpu", "cpu", "Chip"], ["atom", "atom", "Atom"], ["planet", "planet", "Planet"], ["rocket", "rocket-launch", "Rocket"], ["star", "star-four", "Sparkle"], ["lightning", "lightning", "Bolt"], ["flame", "flame", "Flame"], ["leaf", "leaf", "Leaf"], ["snowflake", "snowflake", "Snowflake"], ["moon", "moon-stars", "Night"], ["cat", "cat", "Cat"], ["ghost", "ghost", "Ghost"], ["alien", "alien", "Alien"]]
                        Squircle {
                            id: choice
                            required property var modelData
                            // (the badge was "circle", and "irice" was the drop, Tessera's mark before)
                            readonly property bool on: Config.options.appearance.menuIcon === modelData[0]
                                || (modelData[0] === "tessera" && ["circle", "irice"].includes(Config.options.appearance.menuIcon))
                            readonly property string distroIcon: modelData[0] === "distro" && SystemInfo.osLogo !== "" ? Quickshell.iconPath(SystemInfo.osLogo, true) : ""
                            width: 40
                            height: 32
                            radius: Theme.radius.control
                            color: on ? Theme.accentTint : choiceMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                            border.width: on ? 2 : 0
                            border.color: Theme.accent
                            Icon {
                                anchors.centerIn: parent
                                visible: choice.distroIcon === ""
                                name: choice.modelData[1] || "linux-logo"
                                fill: true
                                size: 18
                                color: Theme.color.label
                            }
                            Image {
                                anchors.centerIn: parent
                                visible: choice.distroIcon !== ""
                                source: choice.distroIcon
                                sourceSize: Qt.size(18, 18)
                            }
                            MouseArea {
                                id: choiceMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Config.options.appearance.menuIcon = choice.modelData[0]
                            }
                            QQC.ToolTip.text: modelData[2]
                            QQC.ToolTip.visible: choiceMouse.containsMouse
                            QQC.ToolTip.delay: 600
                        }
                    }
                }
            }
            Row {
                id: iconThemeRow
                label: "Icon theme"
                detail: "App icons in the dock, App Drawer, Search and Overview. Changing it restarts the shell; apps keep running."
                // Installed icon themes (not cursor themes)
                property var themes: []
                Process {
                    running: true
                    command: ["sh", "-c", "for d in \"${XDG_DATA_HOME:-$HOME/.local/share}/icons\" \"$HOME/.local/share/icons\" \"$HOME/.icons\" /usr/local/share/icons /usr/share/icons; do for t in \"$d\"/*/index.theme; do [ -f \"$t\" ] && grep -q '^Directories=' \"$t\" && basename \"$(dirname \"$t\")\"; done; done | grep -vx hicolor | sort -u"]
                    stdout: StdioCollector {
                        onStreamFinished: iconThemeRow.themes = text.split("\n").filter(x => x !== "")
                    }
                }
                Dropdown {
                    width: 220
                    options: [{ value: "", label: "Automatic" }].concat(iconThemeRow.themes.map(t => ({ value: t, label: t })))
                    value: Config.options.appearance.iconTheme
                    onChosen: v => {
                        if (v === Config.options.appearance.iconTheme)
                            return;
                        Config.options.appearance.iconTheme = v;
                        applyTheme.restart(); // after the setting is saved
                    }
                    Timer {
                        id: applyTheme
                        interval: 800
                        onTriggered: System.repairShell()
                    }
                }
            }
            Row {
                label: "Shortcut symbols"
                detail: "e.g. " + KeyNames.combo("SUPER CTRL F")
                Segmented {
                    width: 300
                    readonly property var values: ["auto", "mac", "windows", "linux"]
                    options: ["Auto", "⌘ macOS", "Windows", "Linux"]
                    current: Math.max(0, values.indexOf(Config.options.appearance.keySymbols))
                    onSelected: i => Config.options.appearance.keySymbols = values[i]
                }
            }
        }
    }
}
