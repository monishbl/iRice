import QtQuick
import QtQuick.Controls as QQC
import Quickshell
import Quickshell.Io
import qs.common
import qs.modules.settings
import qs.services

Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
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
            label: "App icons"
            detail: ({ dark: "Dark backgrounds, colourful artwork", clear: "Frosted and colourless", tinted: "Tinted with the accent colour", auto: "Dark icons in dark mode" })[Config.options.appearance.iconStyle] ?? ""
            Segmented {
                width: 340
                readonly property var values: ["default", "dark", "clear", "tinted", "auto"]
                options: ["Default", "Dark", "Clear", "Tinted", "Auto"]
                current: Math.max(0, values.indexOf(Config.options.appearance.iconStyle))
                onSelected: i => Config.options.appearance.iconStyle = values[i]
            }
        }
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
        Row {
            label: "Emoji"
            detail: EmojiFont.available ? "In the emoji picker (Super+.) and Spotlight" : EmojiFont.packageFor(EmojiFont.style) + " isn't installed; showing Android's until it is"
            Flow { // (Row here is the settings row)
                spacing: 8
                width: (EmojiFont.available ? 0 : emojiInstall.implicitWidth + 32) + 300
                Rectangle {
                    visible: !EmojiFont.available
                    width: emojiInstall.implicitWidth + 24
                    height: 28
                    radius: Theme.radius.control
                    color: emojiInstallMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                    Text {
                        id: emojiInstall
                        anchors.centerIn: parent
                        text: "Install font"
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
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
                    options: ["Android", "Apple", "Windows"]
                    current: Math.max(0, values.indexOf(EmojiFont.style))
                    onSelected: i => Config.options.emoji.style = values[i]
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Menu bar and shortcuts"
        footer: "The icon opens the system menu. Shortcut symbols are how keys are written in menus, Settings and the cheatsheet" + (Config.options.appearance.keySymbols === "auto" ? "; Auto follows the icon: Apple and Command use ⌘ ⌃ ⌥ ⇧, Windows uses Win Ctrl Alt, others Super Ctrl Alt." : ".")
        Row {
            label: "Menu icon"
            Flow { // (Row here is the settings row)
                spacing: 4
                width: 8 * 40 + 7 * 4
                Repeater {
                    model: [["irice", "drop", "iRice"], ["distro", "", (SystemInfo.osName || "Your distro")], ["linux", "linux-logo", "Linux"], ["windows", "windows-logo", "Windows"], ["apple", "apple-logo", "Apple"], ["android", "android-logo", "Android"], ["command", "command", "Command"], ["terminal", "terminal-window", "Terminal"], ["cpu", "cpu", "Chip"], ["atom", "atom", "Atom"], ["planet", "planet", "Planet"], ["rocket", "rocket-launch", "Rocket"], ["star", "star-four", "Sparkle"], ["lightning", "lightning", "Bolt"], ["flame", "flame", "Flame"], ["leaf", "leaf", "Leaf"], ["snowflake", "snowflake", "Snowflake"], ["moon", "moon-stars", "Night"], ["cat", "cat", "Cat"], ["ghost", "ghost", "Ghost"], ["alien", "alien", "Alien"], ["steam", "steam-logo", "Steam"], ["github", "github-logo", "GitHub"], ["circle", "circle-wavy", "Badge"]]
                    Rectangle {
                        id: choice
                        required property var modelData
                        readonly property bool on: Config.options.appearance.menuIcon === modelData[0]
                        readonly property string distroIcon: modelData[0] === "distro" && SystemInfo.osLogo !== "" ? Quickshell.iconPath(SystemInfo.osLogo, true) : ""
                        width: 40
                        height: 32
                        radius: Theme.radius.control
                        color: on ? Theme.accentAlpha(0.25) : choiceMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
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
            detail: "App icons in the dock, App Library, Spotlight and Overview. Changing it restarts the shell; apps keep running."
            // Installed icon themes (not cursor themes)
            property var themes: []
            Process {
                running: true
                command: ["sh", "-c", "for d in \"${XDG_DATA_HOME:-$HOME/.local/share}/icons\" \"$HOME/.icons\" /usr/local/share/icons /usr/share/icons; do for t in \"$d\"/*/index.theme; do [ -f \"$t\" ] && grep -q '^Directories=' \"$t\" && basename \"$(dirname \"$t\")\"; done; done | grep -vx hicolor | sort -u"]
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

    Group {
        width: parent.width
        SubpageLink {
            icon: "drop-half"
            label: "Glass"
            detail: "Material, frost, lens and which apps match it"
            value: ({ liquid: "Glass", frosted: "Frost", solid: "Solid", custom: "Custom" })[Theme.glass.material] ?? ""
            page: "appearance/glass"
        }
        SubpageLink {
            icon: "app-window"
            label: "Windows and Overview"
            detail: "Window corners and borders, Stage Manager, Overview style"
            page: "appearance/windows"
        }
        SubpageLink {
            icon: "thermometer-simple"
            label: "Cool-down and performance"
            detail: "What iRice turns down when the machine runs hot, and what stays loaded"
            page: "appearance/cooldown"
        }
    }

    Group {
        width: parent.width
        title: "Accessibility"
        Row {
            label: "Reduce transparency"
            detail: "Solid surfaces instead of glass, whatever the material above"
            Switch {
                checked: Config.options.appearance.reduceTransparency
                onToggled: Config.options.appearance.reduceTransparency = !checked
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
}
