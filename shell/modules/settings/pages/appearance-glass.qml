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
    readonly property string pageTitle: "Glass"

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
                        background: GlassPopupBackground { radius: Theme.radius.control + 16 }
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
                                    { key: "glassTint", label: "Tint", hint: Theme.dark ? "How much it darkens what's behind" : "How milky it is", max: 2 }
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
                                        to: parent.modelData.max ?? 1 // tint goes to 200%
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
                    onClicked: Launch.terminal(["sh", "-c", "\"$1\" --load", "sh", (Quickshell.env("IRICE_ROOT") || (Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share") + "/irice/src") + "/tools/build-plugin.sh"], true)
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
        Row {
            visible: GlassPlugin.available && Theme.glass.tier !== "solid"
            label: "Frost over windows"
            detail: "Glass covering a window blurs it more, so its text doesn't compete with what's on the glass"
            Segmented {
                width: 300
                readonly property var values: [0, 0.5, 0.75, 0.95]
                options: ["Off", "Subtle", "Medium", "Strong"]
                current: {
                    const v = Config.options.appearance.glassWindowFrost;
                    let best = 0;
                    for (let i = 1; i < values.length; i++)
                        if (Math.abs(values[i] - v) < Math.abs(values[best] - v))
                            best = i;
                    return best;
                }
                onSelected: i => Config.options.appearance.glassWindowFrost = values[i]
            }
        }
    }

    Group {
        width: parent.width
        title: "Match apps"
        footer: !AppTheme.available ? "Install matugen to colour your apps to match." : Config.options.appearance.appTheme === "off" ? "Apps keep their own colours." : "Apps get colours from your " + (Config.options.appearance.appTheme === "wallpaper" ? "wallpaper" : "accent colour") + ", light or dark with iRice. Open apps update when they can; others on their next start. Turning this off removes the colours again."
        Row {
            label: "Colours from"
            Segmented {
                width: 260
                enabled: AppTheme.available
                readonly property var values: ["off", "accent", "wallpaper"]
                options: ["Off", "Accent", "Wallpaper"]
                current: Math.max(0, values.indexOf(Config.options.appearance.appTheme))
                onSelected: i => Config.options.appearance.appTheme = values[i]
            }
        }
        Repeater {
            model: Config.options.appearance.appTheme === "off" ? [] : [["gtk", "GTK apps"], ["qt", "Qt and KDE apps"], ["terminals", "Terminals (foot, kitty, Alacritty)"], ["browsers", "Firefox (with Pywalfox) and Chromium"]]
            Row {
                id: targetRow
                required property var modelData
                label: modelData[1]
                Switch {
                    checked: Config.options.appearance.appThemeTargets.includes(targetRow.modelData[0])
                    onToggled: {
                        const t = Config.options.appearance.appThemeTargets.filter(x => x !== targetRow.modelData[0]);
                        Config.options.appearance.appThemeTargets = checked ? t : t.concat([targetRow.modelData[0]]);
                    }
                }
            }
        }
    }

}
