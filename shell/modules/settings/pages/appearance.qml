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
        title: "Menu bar and shortcuts"
        footer: "The icon opens the system menu. Shortcut symbols are how keys are written in menus, Settings and the cheatsheet" + (Config.options.appearance.keySymbols === "auto" ? "; Auto follows the icon: Apple uses ⌘ ⌃ ⌥ ⇧, Windows uses Win Ctrl Alt, others Super Ctrl Alt." : ".")
        Row {
            label: "Menu icon"
            Flow { // (Row here is the settings row)
                spacing: 4
                width: 6 * 40 + 5 * 4
                Repeater {
                    model: [["irice", "drop", "iRice"], ["distro", "", (SystemInfo.osName || "Your distro")], ["linux", "linux-logo", "Linux"], ["windows", "windows-logo", "Windows"], ["apple", "apple-logo", "Apple"], ["android", "android-logo", "Android"]]
                    Rectangle {
                        id: choice
                        required property var modelData
                        readonly property bool on: Config.options.appearance.menuIcon === modelData[0]
                        readonly property string distroIcon: modelData[0] === "distro" && SystemInfo.osLogo !== "" ? Quickshell.iconPath(SystemInfo.osLogo, true) : ""
                        width: 40
                        height: 32
                        radius: Theme.radius.control
                        color: on ? Qt.alpha(Theme.accent, 0.25) : choiceMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
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
        title: "Windows"
        footer: Config.options.appearance.decorations === "titlebars" && !Decorations.pluginLoaded ? "Title bars need the hyprbars plugin, which isn't installed." : Config.options.appearance.decorations === "controls" ? (Decorations.pluginControls ? "Hover the handle at the top of any window for close, minimize and zoom; rest on the green button to tile. Drawn by the compositor, so it moves with the window." : "Hover the handle at the top of the focused window for close, minimize and zoom. With the compositor glass plugin it's drawn on every window and never lags behind.") : Config.options.appearance.decorations === "titlebars" ? "macOS-style title bars with traffic lights. Double-click a title bar to zoom." : "Borderless windows; use the Window menu for window actions."
        Row {
            label: "Decorations"
            Segmented {
                width: 300
                readonly property var values: ["none", "controls", "titlebars"]
                options: ["None", "Window controls", "Title bars"]
                current: Math.max(0, values.indexOf(Config.options.appearance.decorations))
                onSelected: i => Config.options.appearance.decorations = values[i]
            }
        }
        Row {
            visible: Config.options.appearance.decorations === "titlebars" && !Decorations.pluginLoaded
            label: "hyprbars plugin"
            detail: "Builds the official plugin with hyprpm in a terminal."
            Rectangle {
                width: instLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: Theme.accent
                Text { id: instLabel; anchors.centerIn: parent; text: "Install…"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea { anchors.fill: parent; onClicked: Decorations.installPlugin() }
            }
        }
    }

    Group {
        width: parent.width
        title: "Stage Manager"
        footer: "Keeps one group of windows in front and the others as thumbnails on the left. A new app opens in its own stage; Shift-click a thumbnail to add its windows to the stage in front. Also in Control Center."
        Row {
            label: "Stage Manager"
            Switch {
                checked: Config.options.stageManager.enabled
                onToggled: Config.options.stageManager.enabled = !checked
            }
        }
        Row {
            visible: Config.options.stageManager.enabled
            label: "Show recent stages"
            detail: "Hide them to give windows the full width; switch with the dock or Mission Control."
            Switch {
                checked: Config.options.stageManager.strip
                onToggled: Config.options.stageManager.strip = !checked
            }
        }
    }

    Group {
        width: parent.width
        title: "Glass"
        footer: Theme.glass.lensAmount > 0.5 && !GlassPlugin.available ? "Glass needs the irice-glass Hyprland plugin; until it's built, it looks frosted." : Theme.glass.material === "liquid" ? "Glass bends whatever is behind it, live, with light catching the edges, like Tahoe." : Theme.glass.material === "frosted" ? "Frost blurs what's behind, with a thin lens at the edges." : Theme.glass.material === "solid" ? "Solid turns transparency off, everywhere." : "Your own mix of lens, frost and solid. Advanced… changes it."
        Row {
            label: "Material"
            Flow { // (Row here is the settings row)
                spacing: 8
                width: materialSeg.width + 8 + advButton.width
                Segmented {
                    id: materialSeg
                    width: Theme.glass.material === "custom" ? 300 : 230
                    readonly property var values: ["liquid", "frosted", "solid", "custom"]
                    options: Theme.glass.material === "custom" ? ["Glass", "Frost", "Solid", "Custom"] : ["Glass", "Frost", "Solid"]
                    current: Math.max(0, values.indexOf(Theme.glass.material))
                    onSelected: i => Config.options.appearance.glass = values[i]
                }
                // The three amounts behind the presets, as sliders
                Rectangle {
                    id: advButton
                    y: (materialSeg.height - height) / 2
                    width: advLabel.implicitWidth + 20
                    height: 28
                    radius: 14
                    color: advMouse.containsMouse || advanced.opened ? Theme.color.fillHover : Theme.color.fill
                    Text { id: advLabel; anchors.centerIn: parent; text: "Advanced…"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                    MouseArea {
                        id: advMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: advanced.open()
                    }
                    QQC.Popup {
                        id: advanced
                        y: parent.height + 8
                        x: parent.width - width
                        width: 300
                        padding: 16
                        modal: false
                        margins: 12 // stays inside the Settings window (moves up when near the bottom)
                        closePolicy: QQC.Popup.CloseOnEscape | QQC.Popup.CloseOnPressOutside
                        // Start from whatever is showing, so the sliders don't jump
                        onAboutToShow: {
                            const a = Theme.glass.amounts;
                            Config.options.appearance.glassLens = a[0];
                            Config.options.appearance.glassFrost = a[1];
                            Config.options.appearance.glassSolid = a[2];
                            Config.options.appearance.glassTint = a[3] ?? 0.45;
                        }
                        background: Rectangle {
                            radius: Theme.radius.control + 16
                            color: Theme.dark ? "#232327" : "#FFFFFF" // raised surface
                            border.width: 1
                            border.color: Theme.color.separator
                        }
                        contentItem: Column {
                            spacing: 14
                            Text {
                                text: "Custom material"
                                color: Theme.color.label
                                font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                            }
                            Repeater {
                                model: [
                                    { key: "glassLens", label: "Glass", hint: "How much it bends what's behind" },
                                    { key: "glassFrost", label: "Frost", hint: "How much it blurs" },
                                    { key: "glassSolid", label: "Solid", hint: "How opaque it is" },
                                    { key: "glassTint", label: "Tint", hint: Theme.dark ? "How much it darkens what's behind" : "How milky it is" }
                                ]
                                Column {
                                    required property var modelData
                                    width: 268
                                    spacing: 4
                                    Item {
                                        width: parent.width
                                        height: 18
                                        Text {
                                            text: parent.parent.modelData.label
                                            color: Theme.color.label
                                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                                        }
                                        Text {
                                            anchors.right: parent.right
                                            text: Math.round(Config.options.appearance[parent.parent.modelData.key] * 100) + "%"
                                            color: Theme.color.secondary
                                            font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                                        }
                                    }
                                    Slider {
                                        width: parent.width
                                        from: 0
                                        to: 1
                                        value: Config.options.appearance[parent.modelData.key]
                                        onMoved: v => {
                                            Config.options.appearance[parent.modelData.key] = Math.round(v * 100) / 100;
                                            Config.options.appearance.glass = "custom";
                                        }
                                    }
                                    Text {
                                        text: parent.modelData.hint
                                        color: Theme.color.tertiary
                                        font { family: Theme.font.family; pixelSize: Theme.font.caption }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Row {
            visible: Theme.glass.lensAmount > 0.4 && Theme.glass.solidAmount < 0.9
            label: "Lens"
            Segmented {
                width: 260
                readonly property var values: ["thin", "depth", "auto"]
                options: ["Thin", "Depth", "Auto"]
                current: Math.max(0, values.indexOf(Config.options.appearance.glassDepth))
                onSelected: i => Config.options.appearance.glassDepth = values[i]
            }
        }
        Row {
            visible: Theme.glass.tier !== "solid"
            label: "Compositor glass"
            detail: GlassPlugin.available ? "Active: Hyprland draws the glass" : "Not loaded. Build it once (and again after Hyprland updates)."
            Rectangle {
                visible: !GlassPlugin.available
                width: buildLabel.implicitWidth + 24
                height: 28
                radius: 14
                color: buildMouse.containsMouse ? Qt.darker(Theme.accent, 1.1) : Theme.accent
                Text { id: buildLabel; anchors.centerIn: parent; text: "Build plugin…"; color: Theme.color.onAccent; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold } }
                MouseArea {
                    id: buildMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: Quickshell.execDetached(["foot", "-H", "sh", "-c", "\"$1\" --load", "sh", (Quickshell.env("IRICE_ROOT") || (Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share") + "/irice/src") + "/tools/build-plugin.sh"])
                }
            }
            Icon {
                visible: GlassPlugin.available
                name: "check-circle"
                fill: true
                size: 18
                color: Theme.color.success
            }
        }
        Row {
            label: "Motion"
            detail: Config.options.appearance.motion === "expressive" ? "More pull toward the pointer and bouncier springs, like iOS 26" : "Springs with a touch of overshoot, like macOS"
            Segmented {
                width: 220
                readonly property var values: ["subtle", "expressive"]
                options: ["Subtle", "Expressive"]
                current: Math.max(0, values.indexOf(Config.options.appearance.motion))
                onSelected: i => Config.options.appearance.motion = values[i]
            }
        }
        Row {
            label: "Window corners"
            detail: Config.options.appearance.windowRadius + " px, continuous curve"
            Slider {
                width: 220
                from: 0
                to: 32
                stepSize: 1
                value: Config.options.appearance.windowRadius
                onMoved: v => Config.options.appearance.windowRadius = Math.round(v)
            }
        }
        Row {
            label: "Rounded screen corners"
            Switch {
                checked: Config.options.appearance.screenCorners
                onToggled: Config.options.appearance.screenCorners = !checked
            }
        }
        Row {
            label: "Tint glass with wallpaper colour"
            Switch {
                checked: Config.options.appearance.wallpaperTint
                onToggled: Config.options.appearance.wallpaperTint = !checked
            }
        }
    }
}
