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

    Group {
        width: parent.width
        footer: Config.options.lock.layout === "macos" ? "The clock up top; your picture, name and the password field at the bottom." : "A large clock with small widgets, Now Playing and your notifications, like iPhone."
        Row {
            label: "Layout"
            Choice { key: "layout"; values: ["ios", "macos"]; options: ["iOS", "macOS"] }
        }
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
        Row {
            label: "Show notification previews"
            detail: "Off shows only which app sent each notification"
            Switch { checked: Config.options.lock.previews; onToggled: Config.options.lock.previews = !checked }
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

    Group {
        width: parent.width
        title: "StandBy"
        footer: "While charging and left alone, the screen shows a large clock, your widgets or photos instead of going dark. Swipe or scroll to change; any key brings your desktop back."
        Row {
            label: "StandBy"
            Switch { checked: Config.options.lock.standby; onToggled: Config.options.lock.standby = !checked }
        }
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

    // ── Fingerprint (services/Fingerprint) ──────────────────────────────
    // Only on a computer with a reader: what it needs, the fingers, and
    // where a finger works
    Component.onCompleted: { Fingerprint.refresh(); Face.refresh(); }
    Group {
        width: parent.width
        visible: Fingerprint.hasReader
        title: "Fingerprint"
        footer: !Fingerprint.ready ? "" : "Password prompts, sudo and the login screen wait a few seconds for a finger, then ask for your password. Changing them asks for your password."
            + (Fingerprint.pamError !== "" ? " " + Fingerprint.pamError : "")

        // 1. What the reader still needs
        Row {
            visible: !Fingerprint.ready
            label: Fingerprint.reader?.name ?? "Fingerprint reader"
            detail: Fingerprint.installError !== "" ? Fingerprint.installError
                : Fingerprint.missing.length > 0 ? "Found. To use it, install " + Fingerprint.missing.join(", ") + ((Fingerprint.info.aur ?? []).length > 0 ? " (from the AUR, with yay or paru)" : "") + ". It asks for your password once."
                : "Found, but fprintd can't use it yet. Restart, then open this page again."
            labelColor: Fingerprint.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                visible: Fingerprint.missing.length > 0
                text: Fingerprint.installing ? "Installing…" : "Install"
                icon: "download-simple"
                primary: true
                busy: Fingerprint.installing
                enabled: !Updates.running
                onClicked: Fingerprint.install()
            }
        }

        // 2. The fingers
        Repeater {
            model: Fingerprint.ready ? Fingerprint.enrolled : []
            Row {
                required property string modelData
                label: Fingerprint.fingerNames[modelData] ?? modelData
                SettingsButton {
                    text: "Remove"
                    danger: true
                    onClicked: Fingerprint.remove(parent.parent.modelData ?? "")
                }
            }
        }
        Row {
            id: adding
            visible: Fingerprint.ready
            readonly property bool busy: Fingerprint.enrolling || Fingerprint.enrollStage === "waiting"
            readonly property var free: Object.keys(Fingerprint.fingerNames).filter(f => !Fingerprint.enrolled.includes(f))
            property string pick: free[0] ?? ""
            label: busy ? Fingerprint.enrollHint || "Touch the fingerprint reader" : Fingerprint.enrolled.length === 0 ? "Add a fingerprint" : "Add another"
            detail: busy ? (Fingerprint.enrollStages > 0 ? Fingerprint.enrollCount + " of " + Fingerprint.enrollStages + " · " : "") + (Fingerprint.fingerNames[Fingerprint.enrollFinger] ?? "")
                : Fingerprint.enrollStage === "done" ? "Added. It works on the lock screen now."
                : Fingerprint.enrollStage === "error" ? Fingerprint.enrollError
                : "Touch the reader a few times with the same finger"
            labelColor: !busy && Fingerprint.enrollStage === "error" ? Theme.color.danger : Theme.color.label
            QQ.Row {
                spacing: 8
                Dropdown {
                    visible: !adding.busy
                    width: 190
                    options: adding.free.map(f => ({ value: f, label: Fingerprint.fingerNames[f] }))
                    value: adding.pick
                    onChosen: v => adding.pick = v
                }
                SettingsButton {
                    text: adding.busy ? "Cancel" : "Add"
                    icon: adding.busy ? "" : "fingerprint"
                    primary: !adding.busy
                    enabled: adding.busy || adding.pick !== ""
                    onClicked: adding.busy ? Fingerprint.cancelEnroll() : Fingerprint.enroll(adding.pick)
                }
            }
        }

        // 3. Where a finger works
        Row {
            visible: Fingerprint.ready
            label: "Unlock the lock screen"
            detail: "Touch the reader or type your password"
            Switch { checked: Config.options.lock.fingerprint ?? true; onToggled: Config.options.lock.fingerprint = !checked }
        }
        Repeater {
            model: Fingerprint.ready ? [
                { key: "polkit", label: "Password prompts", detail: "When an app or iRice asks for an administrator's password" },
                { key: "sudo", label: "sudo in terminals", detail: "sudo asks for a finger first" },
                { key: "login", label: "Login screen", detail: "Your password is still needed once after a restart: it unlocks your saved passwords" }
            ] : []
            Row {
                required property var modelData
                label: modelData.label
                detail: Fingerprint.pamBusy === modelData.key ? "Changing…" : modelData.detail
                Switch {
                    checked: Fingerprint.pam[parent.parent.modelData?.key ?? ""] ?? false
                    enabled: Fingerprint.pamBusy === ""
                    onToggled: Fingerprint.setPam(parent.parent.modelData.key, !checked)
                }
            }
        }
    }

    // ── Face (services/Face) ────────────────────────────────────────────
    // Only with an infrared camera; the lock screen only
    Group {
        width: parent.width
        visible: Face.hasCamera
        title: "Face"
        footer: Face.error !== "" ? Face.error
            : !Face.installed ? "Look at the screen to unlock it, with your laptop's infrared camera (" + (Face.camera?.name ?? "") + "). It's less secure than a password or a fingerprint: someone who looks very like you might get in. Your password always works too."
            : "Look at the screen when it's locked. Face unlock is for the lock screen only; password prompts and sudo still ask for your password or finger. Each change asks for your password."
        Row {
            visible: !Face.installed
            label: "Face unlock"
            detail: Face.installError !== "" ? Face.installError : "Needs Howdy (howdy-next, from the AUR, with yay or paru). It asks for your password once."
            labelColor: Face.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: Face.installing ? "Installing…" : "Install"
                icon: "download-simple"
                busy: Face.installing
                enabled: !Updates.running
                onClicked: Face.install()
            }
        }
        Row {
            visible: Face.installed && !Face.setUp
            label: "Set up the camera"
            detail: "Uses " + (Face.camera?.name ?? "the IR camera") + " and downloads Howdy's face models"
            SettingsButton {
                text: Face.busy === "setup" ? "Setting up…" : "Set Up"
                primary: true
                busy: Face.busy === "setup"
                enabled: Face.busy === ""
                onClicked: Face.setUpCamera()
            }
        }
        Row {
            visible: Face.setUp
            label: Face.busy === "add" ? "Look at the camera…" : Face.faces === 0 ? "Add your face" : Face.faces === 1 ? "1 face added" : Face.faces + " faces added"
            detail: Face.busy === "add" ? "Hold still for a moment, in normal light" : Face.faces === 0 ? "Look at the camera for a few seconds" : "Add another (with glasses, say) for better recognition"
            QQ.Row {
                spacing: 8
                SettingsButton {
                    visible: Face.faces > 0
                    text: Face.busy === "clear" ? "Removing…" : "Remove All"
                    danger: true
                    enabled: Face.busy === ""
                    onClicked: Face.clearFaces()
                }
                SettingsButton {
                    text: Face.faces === 0 ? "Add Face" : "Add Another"
                    icon: "smiley"
                    primary: Face.faces === 0
                    busy: Face.busy === "add"
                    enabled: Face.busy === ""
                    onClicked: Face.addFace()
                }
            }
        }
        Row {
            visible: Face.setUp && Face.faces > 0
            label: "Unlock the lock screen"
            detail: "Look at the screen or type your password"
            Switch { checked: Config.options.lock.face ?? true; onToggled: Config.options.lock.face = !checked }
        }
    }

    Group {
        width: parent.width
        Row {
            label: "Preview"
            detail: "Locks the screen now"
            Squircle {
                width: lockText.implicitWidth + 28
                height: 28
                radius: Theme.radius.control
                color: lockMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
                Text { id: lockText; anchors.centerIn: parent; text: "Lock Screen"; color: Theme.color.label; font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium } }
                MouseArea { id: lockMouse; anchors.fill: parent; hoverEnabled: true; onClicked: System.power("lock") }
            }
        }
    }
}
