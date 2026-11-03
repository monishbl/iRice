import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.modules.lock as LockUi
import qs.services

Column {
    id: page
    width: parent?.width ?? 0
    spacing: 22

    component Choice: Segmented {
        required property string key
        required property var values
        width: 300
        current: Math.max(0, values.indexOf(Config.options.lock[key]))
        onSelected: i => Config.options.lock[key] = values[i]
    }

    // ── Clock: Android 17's styles, picked from a carousel ──────────────
    Group {
        width: parent.width
        title: "Clock"

        Item {
            id: carousel
            width: parent.width
            height: 212
            readonly property var faces: [
                { id: "dynamic", name: "Dynamic" },
                { id: "stacked", name: "Stacked" },
                { id: "wide", name: "Wide" },
                { id: "overlap", name: "Overlap" },
                { id: "analog", name: "Analog" },
                { id: "weather", name: "Weather" }
            ]
            readonly property int chosen: Math.max(0, faces.findIndex(f => f.id === Config.options.lock.clockFace))

            ListView {
                id: faceList
                anchors.fill: parent
                anchors.topMargin: 12
                anchors.bottomMargin: 12
                orientation: ListView.Horizontal
                spacing: 8
                clip: true
                model: carousel.faces
                // (the chosen one centred, its neighbours peeking in)
                readonly property real itemWidth: 176
                highlightRangeMode: ListView.StrictlyEnforceRange
                preferredHighlightBegin: (width - itemWidth) / 2
                preferredHighlightEnd: (width + itemWidth) / 2
                highlightMoveDuration: Theme.motion.panel
                snapMode: ListView.SnapToItem
                currentIndex: carousel.chosen
                // (a swipe picks the one it settles on)
                onMovementEnded: Config.options.lock.clockFace = carousel.faces[currentIndex].id
                activeFocusOnTab: true
                Keys.onLeftPressed: Config.options.lock.clockFace = carousel.faces[Math.max(0, carousel.chosen - 1)].id
                Keys.onRightPressed: Config.options.lock.clockFace = carousel.faces[Math.min(carousel.faces.length - 1, carousel.chosen + 1)].id
                Accessible.role: Accessible.List
                Accessible.name: "Clock style: " + carousel.faces[carousel.chosen].name

                delegate: Item {
                    id: card
                    required property var modelData
                    required property int index
                    readonly property bool isChosen: index === carousel.chosen
                    // (smaller and fainter the further from the centre)
                    readonly property real off: Math.abs(x + width / 2 - faceList.contentX - faceList.width / 2) / faceList.itemWidth
                    width: faceList.itemWidth
                    height: faceList.height
                    scale: Math.max(0.84, 1 - off * 0.16)
                    opacity: Math.max(0.55, 1 - off * 0.45)

                    Rectangle {
                        id: frame
                        anchors.fill: parent
                        anchors.margins: 6 // (room for the ring)
                        radius: 28
                        clip: true
                        color: Theme.m3.surfaceContainerHighest
                        Image {
                            anchors.fill: parent
                            source: Wallpaper.source
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(352, 376)
                            asynchronous: true
                            opacity: 0.85
                        }
                        Rectangle {
                            anchors.fill: parent
                            color: Qt.alpha("black", 0.28)
                        }
                        LockUi.LockClock {
                            anchors.centerIn: parent
                            anchors.verticalCenterOffset: -12
                            width: implicitWidth
                            height: implicitHeight
                            face: card.modelData.id
                            time: new Date()
                            size: card.modelData.id === "analog" ? 64 : card.modelData.id === "overlap" ? 54 : card.modelData.id === "dynamic" ? 50 : 58
                            weight: Config.options.lock.clockWeight
                            widthAxis: Config.options.lock.clockWidth
                            round: Config.options.lock.clockRound
                            color: Theme.m3.primaryFixed
                            color2: Theme.m3.primaryFixedDim
                            color3: Theme.m3.tertiaryFixed
                        }
                        Text {
                            anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 14 }
                            text: card.modelData.name
                            color: "white"
                            font { family: Theme.font.family; pixelSize: 13; weight: Font.Medium }
                        }
                    }
                    Rectangle {
                        anchors.fill: frame
                        anchors.margins: -4
                        radius: frame.radius + 4
                        color: "transparent"
                        border.width: 3
                        border.color: Theme.m3.primary
                        visible: card.isChosen
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Config.options.lock.clockFace = card.modelData.id
                    }
                    Accessible.role: Accessible.ListItem
                    Accessible.name: card.modelData.name
                    Accessible.selected: card.isChosen
                }
            }
        }
        Row {
            label: "Size"
            Choice { key: "clockSize"; values: ["large", "small"]; options: ["Large", "Small"] }
        }
        Row {
            label: "Thickness"
            Slider {
                width: 220
                from: 100
                to: 1000
                stepSize: 20
                value: Config.options.lock.clockWeight
                onMoved: v => Config.options.lock.clockWeight = Math.round(v)
            }
        }
    }

    Group {
        width: parent.width
        footer: ({ shelf: "Under the clock: which apps have notifications, and the newest one. Now Playing sits over your account.", column: "Beside the clock, like a tablet: Now Playing and the latest notifications.", icons: "Under the clock, only which apps have notifications. Now Playing sits over your account." })[Config.options.lock.notifications] ?? ""
        Row {
            label: "Notifications"
            Choice { key: "notifications"; values: ["shelf", "column", "icons"]; options: ["Newest", "Beside", "Icons"] }
        }
        Row {
            label: "Show notification previews"
            detail: "Off shows only which app sent each notification"
            Switch { checked: Config.options.lock.previews; onToggled: Config.options.lock.previews = !checked }
        }
    }

    Group {
        width: parent.width
        title: "StandBy"
        footer: "While charging and left alone, the screen shows a large clock, your widgets or photos instead of going dark. Swipe or scroll to change; any key brings your desktop back."
        Row {
            label: "StandBy"
            Switch { checked: Config.options.lock.standby; onToggled: Config.options.lock.standby = !checked }
        }
    }

    // The iRice login screen (greetd): its style; the look's colours and
    // clock follow yours (LoginScreen)
    Group {
        width: parent.width
        title: "Login screen"
        footer: !LoginScreen.installed ? "This computer signs in with another login screen. iRice's comes with ./install.sh --greeter; until then, Preview shows it."
            : LoginScreen.status !== "" ? LoginScreen.status : "Your colours, light or dark and your clock go to the login screen by themselves."
        Row {
            label: "Style"
            detail: ({ look: "Your look, the one you used last: Glass or Material, with your clock and colours", text: "Like ly: the session, your login and the password in a box of text, keyboard first", plain: "Black, your picture, your name and the field" })[Config.options.lock.loginStyle] ?? ""
            Choice { key: "loginStyle"; values: ["look", "text", "plain"]; options: ["Your look", "Text", "Plain"] }
        }
        Row {
            label: "Preview"
            detail: "Shows it on this screen. The password there is irice; Esc closes it."
            SettingsButton {
                text: "Preview Login Screen"
                onClicked: LoginScreen.preview()
            }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Preview"
            detail: "Locks the screen now"
            Squircle { // (Material: a 40 px tonal button)
                width: lockText.implicitWidth + (Theme.material ? 32 : 28)
                height: Theme.material ? 40 : 28
                radius: Theme.material ? height / 2 : Theme.radius.control
                color: Theme.material ? Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, lockMouse.containsMouse ? 0.08 : 0)) : lockMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Text { id: lockText; anchors.centerIn: parent; text: "Lock Screen"; color: Theme.material ? Theme.m3.onSecondaryContainer : Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.material ? 14 : Theme.font.small; weight: Font.Medium } }
                MouseArea { id: lockMouse; anchors.fill: parent; hoverEnabled: true; onClicked: System.power("lock") }
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            Row {
                label: "Cards and field"
                detail: "What notifications, Now Playing and the password field are made of"
                Choice { key: "material"; values: ["glass", "frost", "solid", "system"]; options: ["Glass", "Frost", "Solid", "System"] }
            }
            Row {
                label: "Text"
                detail: "Automatic picks light or dark text for the wallpaper"
                Choice { key: "text"; values: ["light", "dark", "auto"]; options: ["Light", "Dark", "Automatic"] }
            }
        }
        Group {
            width: parent.width
            title: "StandBy"
            Row {
                label: "Prevent burn-in"
                detail: "Moves the clock every minute and turns the screen dark for a minute every 15 minutes"
                enabled: Config.options.lock.standby
                Switch { checked: Config.options.lock.standbyShift; onToggled: Config.options.lock.standbyShift = !checked }
            }
            Row {
                label: "Wake with the mouse"
                detail: "Off: only a key or a click brings the lock screen back"
                enabled: Config.options.lock.standby
                Switch { checked: Config.options.lock.standbyWakeOnMouse; onToggled: Config.options.lock.standbyWakeOnMouse = !checked }
            }
            Row {
                label: "Clock brightness"
                detail: Math.round(Config.options.lock.standbyClockOpacity * 100) + "%"
                enabled: Config.options.lock.standby
                Slider {
                    width: 220
                    from: 0.2
                    to: 1
                    stepSize: 0.05
                    value: Config.options.lock.standbyClockOpacity
                    onMoved: v => Config.options.lock.standbyClockOpacity = v
                }
            }
            Row {
                label: "Clock weight"
                enabled: Config.options.lock.standby
                Choice { key: "standbyClockWeight"; values: ["thin", "regular", "bold"]; options: ["Thin", "Regular", "Bold"]; width: 240 }
            }
        }
        Group {
            width: parent.width
            title: "Transitions"
            Row {
                label: "Unlocking"
                detail: Config.options.lock.unlockTransition === "zoom" ? "The lock lifts away, widgets and icons zoom into place and windows fade in, like iPhone" : Config.options.lock.unlockTransition === "fade" ? "The lock fades away and the desktop fades in" : ""
                Choice { key: "unlockTransition"; values: ["zoom", "fade", "none"]; options: ["Zoom", "Fade", "None"]; width: 240 }
            }
            Row {
                label: "Locking"
                detail: Config.options.lock.lockTransition === "zoom" ? "The lock screen settles into place as it appears" : Config.options.lock.lockTransition === "fade" ? "The clock and cards fade in" : ""
                Choice { key: "lockTransition"; values: ["zoom", "fade", "none"]; options: ["Zoom", "Fade", "None"]; width: 240 }
            }
            Row {
                label: "Into StandBy"
                detail: Config.options.lock.standbyTransition === "morph" ? "The clock glides into place and thins" : ""
                enabled: Config.options.lock.standby
                Choice { key: "standbyTransition"; values: ["morph", "fade", "none"]; options: ["Morph", "Fade", "None"]; width: 240 }
            }
        }
        // Several displays: which show StandBy (the rest go dark), and whether
        // their clocks keep to the same spot
        Group {
            width: parent.width
            visible: Quickshell.screens.length > 1
            enabled: Config.options.lock.standby
            title: "StandBy on displays"
            footer: "Displays that are off stay dark while StandBy shows."
            Row {
                label: "Same clock position"
                detail: "Every display moves its clock to the same spot"
                Switch { checked: Config.options.lock.standbySameClock; onToggled: Config.options.lock.standbySameClock = !checked }
            }
            Repeater {
                model: Quickshell.screens
                Row {
                    id: screenRow
                    required property var modelData
                    readonly property string name: modelData.name
                    readonly property var list: Config.options.lock.standbyScreens
                    // An empty list means every display
                    readonly property bool on: list.length === 0 || list.includes(name)
                    readonly property string model: modelData.model ?? ""
                    label: model !== "" ? model + "  ·  " + name : name
                    detail: modelData.width + " × " + modelData.height
                    Switch {
                        checked: screenRow.on
                        onToggled: {
                            const row = screenRow;
                            const all = Quickshell.screens.map(s => s.name);
                            let next = row.list.length === 0 ? all : row.list.slice();
                            next = row.on ? next.filter(n => n !== row.name) : next.concat([row.name]);
                            // Every display on: back to "all" (so new displays join in)
                            if (all.every(n => next.includes(n)))
                                next = [];
                            Config.options.lock.standbyScreens = next;
                        }
                    }
                }
            }
        }
        Group {
            width: parent.width
            title: "Clock"
            Row {
                label: "Width"
                visible: Config.options.lock.clockFace !== "analog"
                Slider {
                    width: 220
                    from: 25
                    to: 151
                    stepSize: 1
                    value: Config.options.lock.clockWidth
                    onMoved: v => Config.options.lock.clockWidth = Math.round(v)
                }
            }
            Row {
                label: "Digits"
                visible: Config.options.lock.clockFace !== "analog"
                Segmented {
                    width: 200
                    options: ["Rounded", "Sharp"]
                    current: Config.options.lock.clockRound ? 0 : 1
                    onSelected: i => Config.options.lock.clockRound = i === 0
                }
            }
        }
    }
}
