import QtQuick
import QtQuick as QQ
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// Settings → Phone: an Android phone on this computer (services/Phone):
// what it needs, setting the phone up (USB debugging once, then no cable),
// its screen here, and its camera as a webcam
Column {
    id: page
    width: parent?.width ?? 0
    spacing: 24
    readonly property var phone: Phone.phone
    readonly property bool needsTools: !Phone.hasAdb || !Phone.hasScrcpy || !Tools.has("qrencode")
    readonly property bool pairing: ["starting", "waiting", "pairing", "connecting"].includes(Phone.pairStage)
    Component.onCompleted: Phone.checkCam()

    // What it needs
    Group {
        width: parent.width
        visible: page.needsTools
        footer: "scrcpy shows the phone's screen in a window and lets you use it with your mouse and keyboard. Both are Google's and open source tools."
        Row {
            label: "Android tools and scrcpy"
            detail: Phone.installError !== "" ? Phone.installError : "To see and use your phone here. It asks for your password once."
            labelColor: Phone.installError !== "" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: Phone.installing ? "Installing…" : "Install"
                icon: "download-simple"
                primary: true
                busy: Phone.installing
                enabled: !Updates.running
                onClicked: Phone.install(false)
            }
        }
    }

    // The phone, or how to connect one
    Group {
        width: parent.width
        title: "Phone"
        footer: page.phone ? "Drop files on the phone's window to copy them to its Downloads folder. Right-click the phone in the Dock to come back here."
            : "Once, on the phone: Settings → About phone → tap Build number 7 times; then Settings → System → Developer options → turn on USB debugging. Connect it with a cable and tap Allow."
        Row {
            visible: !!page.phone
            label: page.phone?.model ?? ""
            detail: page.phone ? ["Android " + (page.phone.android ?? ""), page.phone.battery !== undefined ? page.phone.battery + "%" + (page.phone.charging ? " charging" : "") : "", page.phone.wireless ? "Wireless" : "USB"].filter(x => x !== "" && x !== "Android ").join(" · ") : ""
            SettingsButton {
                text: Phone.mirroring(page.phone?.serial ?? "") ? "Showing" : "Show Screen"
                icon: "device-mobile"
                primary: !Phone.mirroring(page.phone?.serial ?? "") && !page.needsTools
                enabled: Phone.hasScrcpy
                onClicked: Phone.mirror(page.phone.serial)
            }
        }
        Row {
            visible: !page.phone && !!Phone.waiting
            label: "Allow this computer on your phone"
            detail: "Unlock the phone and tap Allow on the USB debugging prompt (tick Always allow from this computer)."
            labelColor: Theme.accent
        }
        Row {
            visible: !page.phone && !Phone.waiting
            label: "No phone connected"
            detail: Phone.cfg.enabled ? "Connect it with a USB cable, or pair it without one below." : "Looking for phones is off."
        }
    }

    // Without a cable
    Group {
        width: parent.width
        title: "Without a cable"
        footer: "Android 11 or later, on the same Wi-Fi as this computer. Many phones turn wireless debugging off after a restart or a change of Wi-Fi: pair again then, or use the cable."
        Row {
            visible: Needs.checked && !Needs.avahi
            label: "Network discovery is off"
            detail: "Pairing finds the phone with avahi, which isn't running. It asks for your password once."
            SettingsButton {
                text: Needs.installing || Health.fixing === "avahi" ? "Turning on…" : "Turn On"
                busy: Needs.installing || Health.fixing === "avahi"
                onClicked: Needs.turnOnAvahi()
            }
        }
        Row {
            label: Phone.pairStage === "starting" ? "Getting a code…"
                : Phone.pairStage === "waiting" ? "Scan this code on your phone"
                : Phone.pairStage === "pairing" ? "Pairing…"
                : Phone.pairStage === "connecting" ? "Connecting…"
                : Phone.pairStage === "done" ? "Paired"
                : "Pair with a QR code"
            detail: Phone.pairStage === "waiting" ? "On the phone: Developer options → Wireless debugging → Pair device with QR code"
                : Phone.pairStage === "error" ? (Phone.pairError || "Pairing didn't work. Check the phone is on the same Wi-Fi, then try again.")
                : Phone.pairStage === "done" ? "Your phone is connected over Wi-Fi."
                : "Your phone connects over Wi-Fi from then on"
            labelColor: Phone.pairStage === "error" ? Theme.color.danger : Theme.color.label
            SettingsButton {
                text: page.pairing ? "Cancel" : "Pair"
                enabled: Phone.hasAdb
                onClicked: page.pairing ? Phone.cancelPair() : Phone.pair()
            }
        }
        // The code, on white (cameras read dark on light)
        Item {
            visible: Phone.pairStage === "waiting" && Phone.pairQr !== ""
            width: parent.width
            height: visible ? 196 : 0
            Squircle {
                anchors.centerIn: parent
                width: 176
                height: 176
                radius: Theme.radius.module
                color: "white"
                Image {
                    anchors { fill: parent; margins: 10 }
                    source: Phone.pairQr !== "" ? "file://" + Phone.pairQr : ""
                    sourceSize: Qt.size(312, 312)
                    fillMode: Image.PreserveAspectFit
                    smooth: false
                    cache: false
                    Accessible.role: Accessible.Graphic
                    Accessible.name: "QR code to pair your phone"
                }
            }
        }
    }

    // Its camera
    Group {
        width: parent.width
        title: "Webcam"
        footer: Phone.camError !== "" ? Phone.camError
            : "Your phone's camera as a webcam for calls and meetings: choose “Phone camera” in the app. Needs Android 12 or later."
        Row {
            visible: !Phone.camReady
            label: "Phone camera"
            detail: Phone.installError !== "" && !page.needsTools ? Phone.installError
                : !Phone.camModule ? "Needs a virtual camera (v4l2loopback" + (Phone.kernelPackage !== "" ? ", built for your " + Phone.kernelPackage + " kernel" : "") + "). Install it, then set it up. Each asks for your password once."
                : "Set up a virtual camera for it. It asks for your password once."
            SettingsButton {
                visible: !Phone.camModule
                text: Phone.installing ? "Installing…" : "Install"
                busy: Phone.installing
                enabled: !Updates.running
                onClicked: Phone.install(true)
            }
            SettingsButton {
                visible: Phone.camModule
                text: Phone.camSetting ? "Setting up…" : "Set Up"
                busy: Phone.camSetting
                onClicked: Phone.setUpCam()
            }
        }
        Row {
            visible: Phone.camReady
            label: "Use as webcam"
            detail: !page.phone ? "Connect your phone first" : Phone.webcam ? "On: the phone's " + (Phone.cfg.camera === "front" ? "front" : "back") + " camera" : "Off"
            Switch {
                enabled: !!page.phone && Phone.hasScrcpy
                checked: Phone.webcam
                onToggled: Phone.setWebcam(!checked)
            }
        }
        Row {
            visible: Phone.camReady
            label: "Camera"
            Segmented {
                width: 200
                options: ["Back", "Front"]
                current: Phone.cfg.camera === "front" ? 1 : 0
                onSelected: i => {
                    Phone.cfg.camera = i === 1 ? "front" : "back";
                    if (Phone.webcam) {
                        // (again, with the other camera)
                        Phone.setWebcam(false);
                        restart.restart();
                    }
                }
                Timer { id: restart; interval: 800; onTriggered: Phone.setWebcam(true) }
            }
        }
    }

    Group {
        width: parent.width
        title: "Options"
        Row {
            label: "Look for phones"
            detail: "Off: phones aren't looked for, and the rest of this page waits"
            Switch { checked: Phone.cfg.enabled; onToggled: Phone.cfg.enabled = !checked }
        }
        Row {
            label: "Show the phone in the Dock"
            detail: "While it's connected: click it to see its screen"
            Switch { checked: Phone.cfg.dock; onToggled: Phone.cfg.dock = !checked }
        }
        Row {
            label: "Keep the phone's screen off"
            detail: "While it's shown here (saves its battery; it comes back on when you disconnect)"
            Switch { checked: Phone.cfg.screenOff; onToggled: Phone.cfg.screenOff = !checked }
        }
    }
}
