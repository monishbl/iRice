import QtQuick
import Quickshell
import qs.common
import qs.modules.settings
import qs.services

// A Wi-Fi network as a QR code (services/WifiShare): another phone's camera
// joins it. The password is hidden until asked for, and can be copied.
// `done` takes it away (WifiList puts the list back).
Column {
    id: root

    signal done
    width: parent?.width ?? 0
    spacing: 12
    property bool reveal: false

    Item {
        width: parent.width
        height: 28
        IconButton {
            id: back
            anchors.verticalCenter: parent.verticalCenter
            icon: "caret-left"
            tooltip: "Back to networks"
            onClicked: root.done()
        }
        Text {
            anchors { left: back.right; leftMargin: 6; right: parent.right; verticalCenter: parent.verticalCenter }
            text: "Share “" + WifiShare.ssid + "”"
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
    }

    // The code, on white (cameras read dark on light), or what's happening
    Item {
        width: parent.width
        height: 196
        Squircle {
            anchors.centerIn: parent
            width: 188
            height: 188
            radius: Theme.radius.module
            color: "white"
            visible: WifiShare.stage === "ready"
            Image {
                anchors { fill: parent; margins: 10 }
                source: WifiShare.qr !== "" ? "file://" + WifiShare.qr : ""
                sourceSize: Qt.size(336, 336)
                fillMode: Image.PreserveAspectFit
                smooth: false // (sharp modules read better)
                cache: false
                Accessible.role: Accessible.Graphic
                Accessible.name: "QR code for the Wi-Fi network " + WifiShare.ssid
            }
        }
        Icon {
            anchors.centerIn: parent
            visible: WifiShare.stage === "loading"
            name: "circle-notch"
            size: 22
            color: Theme.color.secondary
            RotationAnimation on rotation { running: WifiShare.stage === "loading"; from: 0; to: 360; duration: 800; loops: Animation.Infinite }
        }
        Column {
            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 12 }
            visible: WifiShare.stage === "error"
            spacing: 10
        Text {
            width: parent.width
            text: WifiShare.error
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            color: Theme.color.danger
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        // (qrencode missing: install it from here)
        SettingsButton {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: WifiShare.error.includes("qrencode")
            text: Needs.installing ? "Installing…" : "Install qrencode"
            busy: Needs.installing
            enabled: !Updates.running
            onClicked: Needs.install(["qrencode", "zbar"])
        }
        }
    }

    Text {
        width: parent.width
        visible: WifiShare.stage === "ready"
        text: "Point a phone's camera at the code to join."
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        color: Theme.color.secondary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }

    // The password: hidden until asked for
    Item {
        width: parent.width
        height: 32
        visible: WifiShare.stage === "ready" && WifiShare.password !== ""
        Text {
            id: label
            anchors { left: parent.left; leftMargin: 6; verticalCenter: parent.verticalCenter }
            text: "Password"
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.small }
        }
        Text {
            anchors { left: label.right; leftMargin: 10; right: eye.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
            text: root.reveal ? WifiShare.password : "••••••••"
            elide: Text.ElideRight
            color: Theme.color.label
            font { family: root.reveal ? Theme.font.mono : Theme.font.family; pixelSize: Theme.font.body }
        }
        IconButton {
            id: eye
            anchors { right: copy.left; verticalCenter: parent.verticalCenter }
            icon: root.reveal ? "eye-slash" : "eye"
            tooltip: root.reveal ? "Hide password" : "Show password"
            onClicked: root.reveal = !root.reveal
        }
        IconButton {
            id: copy
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            icon: "copy"
            tooltip: "Copy password"
            onClicked: Quickshell.execDetached(["wl-copy", "--", WifiShare.password])
        }
    }
}
