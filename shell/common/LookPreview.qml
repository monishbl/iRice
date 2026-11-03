import QtQuick
import Quickshell.Widgets
import "MaterialColor.js" as MaterialColor

// A miniature desktop in one of the looks, on your wallpaper and in your
// colours, for choosing between them (Settings → Appearance, the welcome
// screen). Glass: the menu bar, frosted dock and Control Center. Material:
// the taskbar pill, Quick Settings with its tall slider, a cookie clock.
//   LookPreview { width: 320; height: 200; look: "material"; selected: …; onChosen: … }
Item {
    id: root

    property string look: "glass"
    property bool selected: false
    signal chosen

    readonly property bool mat: look === "material"
    // Material's palette, whichever look is shown now
    // (the Material look's seed: its Theme has it; from Glass, the setting,
    // else the wallpaper's tone)
    readonly property string seed: Theme.materialSeed ?? (/^#[0-9a-fA-F]{6}$/.test(Config.options.appearance.materialSeed) ? Config.options.appearance.materialSeed : String(Wallpaper.tone).slice(0, 7))
    readonly property var m: mat ? (Theme.material ? Theme.m3 : MaterialColor.MCU.scheme(seed, Theme.dark, Config.options.appearance.materialStyle, 0)) : ({})
    readonly property real u: width / 100 // one hundredth of the width

    implicitWidth: 320
    implicitHeight: 200
    activeFocusOnTab: true
    Accessible.role: Accessible.RadioButton
    Accessible.name: mat ? "Material" : "Glass"
    Accessible.checked: selected
    Accessible.onPressAction: root.chosen()
    Keys.onPressed: event => {
        if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
            root.chosen();
            event.accepted = true;
        }
    }

    // The chosen one: a ring in the accent, outside the picture
    Rectangle {
        anchors.fill: frame
        anchors.margins: -5
        radius: frame.radius + 5
        color: "transparent"
        border.width: 3
        border.color: Theme.material ? Theme.m3.primary : Theme.accent
        opacity: root.selected ? 1 : mouse.containsMouse ? 0.35 : 0
        Behavior on opacity { NumberAnimation { duration: Theme.motion.hover } }
    }

    ClippingRectangle {
        id: frame
        anchors.fill: parent
        radius: root.mat ? 16 : 12
        color: "black"
        scale: mouse.pressed ? 0.98 : 1
        Behavior on scale { NumberAnimation { duration: Theme.motion.press; easing.type: Easing.OutCubic } }

        Image {
            anchors.fill: parent
            source: Wallpaper.source
            sourceSize.width: 640
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
        }

        // ── Glass ──
        Item {
            id: glassLayer
            anchors.fill: parent
            visible: !root.mat
            readonly property color glassFill: Theme.dark ? Qt.rgba(0.12, 0.12, 0.14, 0.42) : Qt.rgba(1, 1, 1, 0.42)
            readonly property color rim: Qt.rgba(1, 1, 1, Theme.dark ? 0.28 : 0.6)
            readonly property color ink: Theme.dark ? "white" : "#1C1C1E"
            // menu bar: the logo, the app's name and menus, then the status icons and clock
            Rectangle {
                width: parent.width
                height: 4.4 * root.u
                color: Qt.rgba(0, 0, 0, Theme.dark ? 0.18 : 0.05)
                Row {
                    x: 2 * root.u
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 1.6 * root.u
                    Repeater {
                        model: [1.6, 5, 3.4, 3.4, 3]
                        Rectangle {
                            required property real modelData
                            required property int index
                            width: modelData * root.u
                            height: index === 0 ? 1.6 * root.u : 1.1 * root.u
                            radius: height / 2
                            color: glassLayer.ink
                            opacity: index === 1 ? 0.9 : 0.7
                        }
                    }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 2 * root.u; verticalCenter: parent.verticalCenter }
                    spacing: 1.4 * root.u
                    Repeater {
                        model: [1.4, 1.4, 1.4, 7]
                        Rectangle {
                            required property real modelData
                            width: modelData * root.u
                            height: 1.2 * root.u
                            radius: height / 2
                            color: glassLayer.ink
                            opacity: 0.8
                        }
                    }
                }
            }
            // a desktop widget: frosted, with the time
            Squircle {
                x: 4 * root.u
                y: 9 * root.u
                width: 22 * root.u
                height: 22 * root.u
                radius: 5 * root.u
                color: glassLayer.glassFill
                border.width: 1
                border.color: glassLayer.rim
                Text {
                    anchors.centerIn: parent
                    text: Qt.formatTime(clock.now, TimeFormat.hmShort)
                    color: glassLayer.ink
                    font { family: "Inter"; pixelSize: Math.max(8, 6.4 * root.u); weight: Font.DemiBold }
                }
            }
            // Control Center, top right
            Squircle {
                id: cc
                x: 63 * root.u
                y: 6.5 * root.u
                width: 33 * root.u
                height: 31 * root.u
                radius: 4.5 * root.u
                color: glassLayer.glassFill
                border.width: 1
                border.color: glassLayer.rim
                Column {
                    x: 2 * root.u
                    y: 2 * root.u
                    spacing: 1.6 * root.u
                    Repeater {
                        model: 2
                        Squircle {
                            width: 14 * root.u
                            height: 6.4 * root.u
                            radius: 3.2 * root.u
                            color: Qt.rgba(1, 1, 1, Theme.dark ? 0.12 : 0.5)
                            Rectangle {
                                x: 1.2 * root.u
                                anchors.verticalCenter: parent.verticalCenter
                                width: 4.2 * root.u
                                height: width
                                radius: width / 2
                                color: "white"
                                Rectangle {
                                    anchors.centerIn: parent
                                    width: parent.width * 0.42
                                    height: width
                                    radius: width / 2
                                    color: Theme.accent
                                }
                            }
                        }
                    }
                    Row {
                        spacing: 1.6 * root.u
                        Repeater {
                            model: 4
                            Rectangle {
                                required property int index
                                width: 6.2 * root.u
                                height: width
                                radius: width / 2
                                color: index === 1 ? "white" : Qt.rgba(1, 1, 1, Theme.dark ? 0.14 : 0.45)
                            }
                        }
                    }
                    Rectangle {
                        width: 29 * root.u
                        height: 5 * root.u
                        radius: height / 2
                        color: Qt.rgba(1, 1, 1, Theme.dark ? 0.14 : 0.45)
                        Rectangle {
                            width: parent.width * 0.6
                            height: parent.height
                            radius: height / 2
                            color: "white"
                        }
                    }
                }
                // the media tile, right
                Squircle {
                    x: 17.4 * root.u
                    y: 2 * root.u
                    width: 13.6 * root.u
                    height: 14.4 * root.u
                    radius: 3.2 * root.u
                    color: Qt.rgba(1, 1, 1, Theme.dark ? 0.12 : 0.5)
                }
            }
            // the dock
            Squircle {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 2.4 * root.u
                width: dockRow.width + 3 * root.u
                height: 9.6 * root.u
                radius: 3.2 * root.u
                color: glassLayer.glassFill
                border.width: 1
                border.color: glassLayer.rim
                Row {
                    id: dockRow
                    anchors.centerIn: parent
                    spacing: 1.2 * root.u
                    Repeater {
                        model: ["#0A84FF", "#30D158", "#FF9F0A", "#FF375F", "#BF5AF2", "#8E8E93"]
                        Squircle {
                            required property string modelData
                            width: 7 * root.u
                            height: width
                            radius: width * 0.23
                            color: modelData
                        }
                    }
                }
            }
        }

        // ── Material ──
        Loader {
            anchors.fill: parent
            active: root.mat
            sourceComponent: Item {
                // top bar: the clock left, status icons right
                Item {
                    width: parent.width
                    height: 4.4 * root.u
                    Text {
                        x: 2.4 * root.u
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatTime(clock.now, TimeFormat.hmShort)
                        color: Theme.dark ? "white" : root.m.onSurface
                        font { family: "Google Sans Flex"; pixelSize: Math.max(6, 2.6 * root.u); weight: Font.Medium }
                    }
                    Row {
                        anchors { right: parent.right; rightMargin: 2.4 * root.u; verticalCenter: parent.verticalCenter }
                        spacing: 1.2 * root.u
                        Repeater {
                            model: [1.6, 1.6, 2.8]
                            Rectangle {
                                required property real modelData
                                width: modelData * root.u
                                height: 1.6 * root.u
                                radius: 0.5 * root.u
                                color: Theme.dark ? "white" : root.m.onSurface
                                opacity: 0.85
                            }
                        }
                    }
                }
                // a clock widget in a cookie
                MaterialShape {
                    x: 4 * root.u
                    y: 8 * root.u
                    width: 24 * root.u
                    height: width
                    shape: "cookie9"
                    animated: false
                    color: root.m.primaryContainer ?? "#EADDFF"
                    Text {
                        anchors.centerIn: parent
                        text: Qt.formatTime(clock.now, TimeFormat.hmShort)
                        color: root.m.onPrimaryContainer ?? "#21005D"
                        font { family: "Google Sans Flex"; pixelSize: Math.max(8, 6 * root.u); weight: Font.DemiBold }
                    }
                }
                // Quick Settings, top right
                Rectangle {
                    x: 63 * root.u
                    y: 6.5 * root.u
                    width: 33 * root.u
                    height: 33 * root.u
                    radius: 4.8 * root.u
                    color: root.m.surfaceContainer ?? "#211F26"
                    Column {
                        x: 2 * root.u
                        y: 2 * root.u
                        spacing: 1.4 * root.u
                        // the tall brightness slider: icon in the track, a thin handle
                        Item {
                            width: 29 * root.u
                            height: 5.6 * root.u
                            Rectangle {
                                width: 17 * root.u
                                height: parent.height
                                topLeftRadius: 2.8 * root.u
                                bottomLeftRadius: 2.8 * root.u
                                topRightRadius: 0.6 * root.u
                                bottomRightRadius: 0.6 * root.u
                                color: root.m.primary ?? "#D0BCFF"
                            }
                            Rectangle {
                                x: 17.9 * root.u
                                width: 0.8 * root.u
                                height: parent.height
                                radius: width / 2
                                color: root.m.primary ?? "#D0BCFF"
                            }
                            Rectangle {
                                x: 19.6 * root.u
                                width: parent.width - x
                                height: parent.height
                                topRightRadius: 2.8 * root.u
                                bottomRightRadius: 2.8 * root.u
                                topLeftRadius: 0.6 * root.u
                                bottomLeftRadius: 0.6 * root.u
                                color: root.m.secondaryContainer ?? "#4A4458"
                            }
                        }
                        // big tiles: one on, one off
                        Row {
                            spacing: 1.2 * root.u
                            Repeater {
                                model: 2
                                Rectangle {
                                    required property int index
                                    width: 13.9 * root.u
                                    height: 6.6 * root.u
                                    radius: height / 2
                                    color: index === 0 ? (root.m.primaryContainer ?? "#4F378B") : (root.m.surfaceContainerHighest ?? "#36343B")
                                    Rectangle {
                                        x: 1.2 * root.u
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 4.2 * root.u
                                        height: width
                                        radius: width / 2
                                        color: parent.index === 0 ? (root.m.primary ?? "#D0BCFF") : (root.m.surfaceContainerHigh ?? "#2B2930")
                                    }
                                }
                            }
                        }
                        // small tiles: rounded squares, one on
                        Row {
                            spacing: 1.2 * root.u
                            Repeater {
                                model: 4
                                Rectangle {
                                    required property int index
                                    width: 6.35 * root.u
                                    height: width
                                    radius: index === 2 ? width / 2 : 1.8 * root.u
                                    color: index === 2 ? (root.m.tertiaryContainer ?? "#633B48") : (root.m.surfaceContainerHighest ?? "#36343B")
                                }
                            }
                        }
                        // media: art in a shape, a wavy line
                        Rectangle {
                            width: 29 * root.u
                            height: 7.4 * root.u
                            radius: 2.4 * root.u
                            color: root.m.surfaceContainerHigh ?? "#2B2930"
                            MaterialShape {
                                x: 1.2 * root.u
                                anchors.verticalCenter: parent.verticalCenter
                                width: 5.4 * root.u
                                height: width
                                shape: "sunny"
                                animated: false
                                color: root.m.tertiary ?? "#EFB8C8"
                            }
                            Rectangle {
                                x: 8 * root.u
                                anchors.verticalCenter: parent.verticalCenter
                                width: 18 * root.u
                                height: 0.8 * root.u
                                radius: height / 2
                                color: root.m.primary ?? "#D0BCFF"
                            }
                        }
                    }
                }
                // the taskbar pill
                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 2.4 * root.u
                    width: barRow.width + 4 * root.u
                    height: 9 * root.u
                    radius: height / 2
                    color: root.m.surfaceContainer ?? "#211F26"
                    Row {
                        id: barRow
                        anchors.centerIn: parent
                        spacing: 1.4 * root.u
                        Repeater {
                            model: ["primaryContainer", "tertiaryContainer", "secondaryContainer", "primary", "tertiary"]
                            Item {
                                required property string modelData
                                required property int index
                                width: 6 * root.u
                                height: 9 * root.u
                                Rectangle {
                                    anchors.centerIn: parent
                                    width: parent.width
                                    height: width
                                    radius: width / 2
                                    color: root.m[parent.modelData] ?? "#4F378B"
                                }
                                Rectangle {
                                    visible: parent.index < 2
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.bottom: parent.bottom
                                    anchors.bottomMargin: 0.4 * root.u
                                    width: 2 * root.u
                                    height: 0.5 * root.u
                                    radius: height / 2
                                    color: parent.index === 0 ? (root.m.primary ?? "#D0BCFF") : (root.m.onSurfaceVariant ?? "#CAC4D0")
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    QtObject {
        id: clock
        property date now: new Date()
    }
    Timer {
        interval: 30000
        repeat: true
        running: root.visible
        onTriggered: clock.now = new Date()
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.chosen()
    }
    FocusRing {
        radius: frame.radius + 4
    }
}
