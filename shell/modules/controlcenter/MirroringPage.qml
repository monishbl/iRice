import QtQuick
import Quickshell
import qs.common
import qs.services

// Screen Mirroring (services/Mirroring): TVs nearby; pick one to show this
// screen on it, pick it again to stop
DetailPage {
    id: root

    title: "Screen Mirroring"

    property bool lookingHere: false
    onVisibleChanged: look(visible)
    Component.onCompleted: look(visible)
    Component.onDestruction: look(false)
    function look(on) {
        if (on !== lookingHere) {
            lookingHere = on;
            Mirroring.look(on);
        }
    }

    Repeater {
        model: ScriptModel { values: Mirroring.displays }
        ListRow {
            required property var modelData
            readonly property bool here: Mirroring.target === modelData.uuid
            icon: modelData.protocol === "chromecast" ? "television-simple" : "screencast"
            highlighted: here && Mirroring.state_ === "streaming"
            title: modelData.name
            status: here ? (Mirroring.state_ === "streaming" ? "Mirroring" : "Connecting…") : Mirroring.protocolName(modelData.protocol)
            busy: here && Mirroring.state_ === "connecting"
            trailingIcon: here ? "stop" : ""
            onClicked: here ? Mirroring.stop() : Mirroring.start(modelData)
        }
    }

    Text {
        visible: Mirroring.displays.length === 0 || Mirroring.error !== "" || !Mirroring.p2p || (Needs.checked && !Needs.networkManager)
        width: parent.width
        leftPadding: 6
        rightPadding: 6
        topPadding: 4
        wrapMode: Text.WordWrap
        text: !Mirroring.available ? "Screen mirroring needs GNOME Network Displays. Install it in Settings → Displays."
            : Mirroring.error !== "" ? Mirroring.error
            : Needs.checked && !Needs.networkManager ? "Miracast TVs need NetworkManager (for Wi-Fi Direct), which isn't running here. Chromecasts and Miracast over the network still work."
            : Mirroring.displays.length === 0 ? "Looking for TVs… Turn on the TV's screen mirroring (Miracast, Screen Mirroring or Cast), nearby."
            : "This computer's Wi-Fi can't do Wi-Fi Direct: Miracast TVs need it. Chromecasts and network Miracast still work."
        color: Mirroring.error !== "" ? Theme.color.danger : Theme.color.tertiary
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
}
