import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
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

    // Material: the clock, Android 17's styles picked from a carousel
    // (ClockFaces), its size and thickness
    Group {
        visible: Theme.material
        width: parent.width
        title: "Clock"
        Loader {
            width: parent.width
            active: Theme.material
            sourceComponent: Component {
                ClockFaces {}
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

    // (in Material, the layout says what each does in its detail, and what
    // the notifications do is the footer)
    Group {
        width: parent.width
        footer: Theme.material ? ({ shelf: "Under the clock: which apps have notifications, and the newest one. Now Playing sits over your account.", column: "Beside the clock, like a tablet: Now Playing and the latest notifications.", icons: "Under the clock, only which apps have notifications. Now Playing sits over your account." })[Config.options.lock.notifications] ?? ""
            : Config.options.lock.layout === "macos" ? "The clock up top; your picture, name and the password field at the bottom." : "A large clock with small widgets, Now Playing and your notifications, like iPhone."
        Row {
            label: "Layout"
            detail: Theme.material ? (Config.options.lock.layout === "macos" ? "The clock and the date, nothing under them" : "Weather, your next event and to-dos under the clock; the battery in the corner") : ""
            Choice { key: "layout"; values: ["ios", "macos"]; options: ["Widgets", "Classic"] }
        }
        Row {
            label: "Notifications"
            detail: Theme.material ? "" : ({ shelf: "Under the clock: which apps have notifications, and the newest one. Now Playing sits over the sign-in.", column: "Beside the clock on a wide screen: Now Playing and the latest notifications.", icons: "Under the clock, only which apps have notifications" })[Config.options.lock.notifications] ?? ""
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
        title: "Bedside Mode"
        footer: "While charging and left alone, the screen shows a large clock, your widgets or photos instead of going dark. Swipe or scroll to change; any key brings your desktop back."
        Row {
            label: "Bedside Mode"
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

    // While the computer starts (Plymouth's theme: BootSplash), and as the
    // desktop first shows after logging in (LoginIntro)
    Group {
        width: parent.width
        title: "Starting up"
        Component.onCompleted: BootSplash.refresh()
        footer: BootSplash.error !== "" ? BootSplash.error
            : BootSplash.busy ? "Rebuilding the boot image. This takes about a minute; you can keep working."
            : BootSplash.blocked !== "" ? BootSplash.blocked
            : BootSplash.done ? "Done. It shows the next time your computer starts."
            : BootSplash.checked && !BootSplash.splash ? "Your boot settings hide the animation: add splash to GRUB_CMDLINE_LINUX_DEFAULT in /etc/default/grub, then run sudo grub-mkconfig -o /boot/grub/grub.cfg."
            : (BootSplash.designs.find(d => d.id === BootSplash.chosen)?.description ?? (BootSplash.chosen === "none" ? "No picture: the boot messages, as text." : "Your system's own" + (BootSplash.original !== "" ? " (" + BootSplash.original + ")." : "."))) + " Changing it asks for your password."
        Row {
            label: "Boot animation"
            detail: "What shows while your computer starts and shuts down"
        }
        BootAnimations {}
        Row {
            label: "Login intro"
            detail: Config.options.appearance.reduceMotion ? "Off while Reduce motion is on (Accessibility)"
                : Theme.material ? "As your desktop first appears after you log in, it opens out of the iRice shape"
                : "As your desktop first appears after you log in, the wallpaper comes into focus and the menu bar and dock settle in"
            QQ.Row {
                spacing: 12
                SettingsButton {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Play Intro"
                    enabled: !Config.options.appearance.reduceMotion
                    onClicked: LockFx.playIntro()
                }
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    accessibleName: "Login intro"
                    checked: Config.options.lock.loginIntro
                    onToggled: Config.options.lock.loginIntro = !checked
                }
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
                visible: !Theme.material // (Material's lock draws its own tonal cards)
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
            title: "Bedside Mode"
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
                label: "Into Bedside Mode"
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
            title: "Bedside Mode on displays"
            footer: "Displays that are off stay dark while Bedside Mode shows."
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
            visible: Theme.material // (the Material lock clock's axes)
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
