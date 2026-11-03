import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Fingerprint and Face unlock (a part of Privacy & Security): the reader or
// the infrared camera, what they still need, the fingers and faces, and
// where they work
Column {
    id: page
    // A fingerprint or the faces, about to go (they can't be brought back:
    // asked once more, by name)
    property string confirming: "" // a finger | "faces"
    width: parent?.width ?? 0
    spacing: 22

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
                : Fingerprint.reader?.unsupported ? "Found, but it has no Linux driver yet (libfprint). Your password works as always; fprint.freedesktop.org lists the readers that do."
                : Fingerprint.missing.length > 0 ? "Found. To use it, install " + Fingerprint.missing.join(", ") + ((Fingerprint.info.aur ?? []).length > 0 ? " (from the AUR: iRice sets up an AUR helper first if there's none)" : "") + ". It asks for your password once."
                : Fingerprint.fixing ? "Setting up the reader. Its firmware is updating; this takes about a minute."
                : Fingerprint.fixError !== "" ? Fingerprint.fixError
                : Fingerprint.slowStart ? "Found. Its driver updates the reader's firmware the first time, which takes about a minute, and the system stops it after 15 seconds. Set Up gives it the time it needs."
                : "Found, but fprintd can't use it yet. Restart, then open this page again."
            labelColor: Fingerprint.installError !== "" || Fingerprint.fixError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                visible: Fingerprint.slowStart && Fingerprint.missing.length === 0
                text: Fingerprint.fixing ? "Setting up…" : "Set Up Reader"
                icon: "fingerprint"
                primary: true
                busy: Fingerprint.fixing
                onClicked: Fingerprint.fix()
            }
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
                id: fingerRow
                required property string modelData
                readonly property bool asking: page.confirming === modelData
                icon: "fingerprint"
                label: Fingerprint.fingerNames[modelData] ?? modelData
                detail: asking ? "It's deleted from the reader: you'd add it again to use it." : ""
                QQ.Row {
                    spacing: 8
                    SettingsButton {
                        visible: fingerRow.asking
                        text: "Keep"
                        onClicked: page.confirming = ""
                    }
                    SettingsButton {
                        text: fingerRow.asking ? "Remove " + (Fingerprint.fingerNames[fingerRow.modelData] ?? "fingerprint") : "Remove"
                        danger: true
                        onClicked: {
                            if (!fingerRow.asking) {
                                page.confirming = fingerRow.modelData;
                                return;
                            }
                            page.confirming = "";
                            Fingerprint.remove(fingerRow.modelData);
                        }
                    }
                }
            }
        }
        Row {
            id: adding
            visible: Fingerprint.ready
            readonly property bool busy: Fingerprint.enrolling || Fingerprint.enrollStage === "waiting"
            readonly property var free: Object.keys(Fingerprint.fingerNames).filter(f => !Fingerprint.enrolled.includes(f))
            property string pick: ""
            // (the one picked, while it's still free: once added, the next)
            readonly property string chosen: free.includes(pick) ? pick : (free[0] ?? "")
            // (adding happens in the sheet: FingerprintSheet)
            label: Fingerprint.enrolled.length === 0 ? "Add a fingerprint" : "Add another"
            detail: "The reader is read 11 times, a different part of your finger each time"
            QQ.Row {
                spacing: 8
                Dropdown {
                    width: 190
                    options: adding.free.map(f => ({ value: f, label: Fingerprint.fingerNames[f] }))
                    value: adding.chosen
                    onChosen: v => adding.pick = v
                }
                SettingsButton {
                    text: "Add"
                    icon: "fingerprint"
                    primary: true
                    enabled: !adding.busy && adding.chosen !== ""
                    onClicked: enrollSheet.open(adding.chosen)
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
                { key: "login", label: "Login screen", detail: Fingerprint.info.gdm ? "GDM asks for a finger by itself. For greetd, ly, SDDM, LightDM or LXDM: your password is still needed once after a restart (it unlocks your saved passwords)" : "Your password is still needed once after a restart: it unlocks your saved passwords" }
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

    // Adding a finger: a sheet over the whole Settings window
    FingerprintSheet {
        id: enrollSheet
        onAddAnother: {
            const next = adding.free.find(f => f !== finger) ?? "";
            if (next !== "")
                open(next);
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
            detail: Face.installError !== "" ? Face.installError : "Needs Howdy (howdy-next, from the AUR: iRice sets up an AUR helper first if there's none). It asks for your password once."
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
                    visible: page.confirming === "faces"
                    text: "Keep"
                    onClicked: page.confirming = ""
                }
                SettingsButton {
                    visible: Face.faces > 0
                    text: Face.busy === "clear" ? "Removing…" : page.confirming === "faces" ? (Face.faces === 1 ? "Remove your face" : "Remove all " + Face.faces + " faces") : "Remove All"
                    danger: true
                    enabled: Face.busy === ""
                    onClicked: {
                        if (page.confirming !== "faces") {
                            page.confirming = "faces";
                            return;
                        }
                        page.confirming = "";
                        Face.clearFaces();
                    }
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
}
