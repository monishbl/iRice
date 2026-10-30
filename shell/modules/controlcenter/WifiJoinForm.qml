import QtQuick
import qs.common
import qs.services

// The full join form on the Wi-Fi page: enterprise networks (802.1X: the
// sign-in method, username and password, and how the network's certificate
// is checked; your own certificate for TLS), and networks that hide their
// name. A saved profile fills it in again. The joining itself is
// WifiJoin's (NetworkManager, passwords on stdin only).
//   WifiJoinForm { ssid: "eduroam"; security: "enterprise"; onDone: … }
Column {
    id: root

    property string ssid: "" // fixed for a network from the list, typed for a hidden one
    property bool hidden: false // "Join other network…": the name is typed, the network may hide it
    property string security: "enterprise" // enterprise | psk | open
    signal done

    // The form's values
    property string name: ssid
    property string eap: "peap"
    property string phase2: "mschapv2"
    property string identity: ""
    property string password: ""
    property string anonymous: ""
    property string domain: ""
    property string ca: "system" // system | none | file
    property string caFile: ""
    property string clientCert: ""
    property string privateKey: ""
    property string keyPassword: ""
    property string psk: ""
    property string problem: "" // a missing value, said before anything is sent

    readonly property bool tls: eap === "tls"
    readonly property bool inner: eap === "peap" || eap === "ttls"
    readonly property bool mine: WifiJoin.ssid === (hidden ? name : ssid)
    readonly property bool joining: WifiJoin.busy && mine
    readonly property string failure: mine && WifiJoin.stage === "error" ? WifiJoin.error : ""

    spacing: 12
    width: parent?.width ?? 0

    // A network already saved: its settings again (never its secrets)
    Component.onCompleted: {
        WifiJoin.dismiss();
        if (ssid !== "")
            WifiJoin.saved(ssid, r => {
                if (!r.exists || r.keyMgmt !== "wpa-eap")
                    return;
                root.eap = ["peap", "ttls", "tls", "pwd"].includes(r.eap) ? r.eap : root.eap;
                root.phase2 = r.phase2 || root.phase2;
                root.identity = r.identity ?? "";
                root.anonymous = r.anonymous ?? "";
                root.domain = r.domain ?? "";
                root.ca = r.caFile ? "file" : r.systemCa === "yes" ? "system" : "none";
                root.caFile = r.caFile ?? "";
                root.clientCert = r.clientCert ?? "";
                root.privateKey = r.privateKey ?? "";
            });
    }
    // Joined: back to the list
    Connections {
        target: WifiJoin
        function onStageChanged() {
            if (WifiJoin.stage === "connected" && root.mine)
                root.done();
        }
    }

    function fileName(path) {
        return path ? path.split("/").pop() : "";
    }
    function choose(title, filter, set) {
        FilePicker.pick({ title: title, start: "~", filter: filter }, path => set(path));
    }
    function submit() {
        problem = "";
        const n = (hidden ? name : ssid).trim();
        if (n === "")
            problem = "Type the network name.";
        else if (security === "psk" && (psk.length < 8 || psk.length > 63))
            problem = "Wi-Fi passwords are 8 to 63 characters.";
        else if (security === "enterprise" && identity.trim() === "")
            problem = "Type your username (often your email address).";
        else if (security === "enterprise" && !tls && password === "")
            problem = "Type your password.";
        else if (security === "enterprise" && tls && (clientCert === "" || privateKey === ""))
            problem = "Choose your certificate and private key files.";
        else if (security === "enterprise" && ca === "file" && caFile === "")
            problem = "Choose the CA certificate file, or use the system's certificates.";
        if (problem !== "")
            return;
        WifiJoin.join({
            ssid: n, hidden: hidden, security: security,
            eap: eap, phase2: phase2, identity: identity, anonymous: anonymous, domain: domain,
            ca: ca === "file" ? caFile : ca, clientCert: clientCert, privateKey: privateKey,
            password: tls ? "" : password, keyPassword: keyPassword, psk: psk
        });
        // (the typed secrets aren't kept once they're sent)
        password = "";
        keyPassword = "";
        psk = "";
    }

    // ── Title ───────────────────────────────────────────────────────────
    Column {
        width: parent.width
        spacing: 2
        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.hidden ? "Join other network" : root.ssid
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: root.hidden ? "For a network that doesn't show its name." : "This network needs a username and password, or a certificate."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }

    // ── Hidden network: its name and security ───────────────────────────
    Field {
        visible: root.hidden
        label: "Network name"
        TextField {
            width: parent.width
            placeholder: "Name"
            text: root.name
            commitOnBlur: false
            onTextChanged: root.name = text
        }
    }
    Field {
        visible: root.hidden
        label: "Security"
        Segmented {
            width: parent.width
            readonly property var values: ["open", "psk", "enterprise"]
            options: ["None", "Password", "Enterprise"]
            current: Math.max(0, values.indexOf(root.security))
            onSelected: i => root.security = values[i]
        }
    }
    Field {
        visible: root.hidden && root.security === "psk"
        label: "Password"
        TextField {
            width: parent.width
            secret: true
            commitOnBlur: false
            placeholder: "8 to 63 characters"
            text: root.psk
            onTextChanged: root.psk = text
            onCommitted: root.submit()
        }
    }

    // ── Enterprise ──────────────────────────────────────────────────────
    Field {
        visible: root.security === "enterprise"
        label: "Sign-in method"
        Segmented {
            width: parent.width
            readonly property var values: ["peap", "ttls", "tls", "pwd"]
            options: ["PEAP", "TTLS", "TLS", "PWD"]
            current: Math.max(0, values.indexOf(root.eap))
            onSelected: i => root.eap = values[i]
        }
    }
    Field {
        visible: root.security === "enterprise" && root.inner
        label: "Inner authentication"
        Dropdown {
            width: parent.width
            options: [{ value: "mschapv2", label: "MSCHAPv2 (most networks)" }, { value: "pap", label: "PAP" }, { value: "gtc", label: "GTC" }, { value: "mschap", label: "MSCHAP" }]
            value: root.phase2
            onChosen: v => root.phase2 = v
        }
    }
    Field {
        visible: root.security === "enterprise"
        label: "Username"
        TextField {
            width: parent.width
            commitOnBlur: false
            placeholder: "you@example.org"
            text: root.identity
            onTextChanged: root.identity = text
        }
    }
    Field {
        visible: root.security === "enterprise" && !root.tls
        label: "Password"
        TextField {
            width: parent.width
            secret: true
            commitOnBlur: false
            text: root.password
            onTextChanged: root.password = text
            onCommitted: root.submit()
        }
    }
    Field {
        visible: root.security === "enterprise" && root.tls
        label: "Your certificate"
        FileButton {
            path: root.clientCert
            onPick: root.choose("Choose your certificate", "Certificates|*.pem *.crt *.cer *.der *.p12 *.pfx", p => root.clientCert = p)
        }
    }
    Field {
        visible: root.security === "enterprise" && root.tls
        label: "Private key"
        FileButton {
            path: root.privateKey
            onPick: root.choose("Choose your private key", "Keys|*.pem *.key *.der *.p12 *.pfx", p => root.privateKey = p)
        }
    }
    Field {
        visible: root.security === "enterprise" && root.tls
        label: "Private key password"
        TextField {
            width: parent.width
            secret: true
            commitOnBlur: false
            placeholder: "If the key has one"
            text: root.keyPassword
            onTextChanged: root.keyPassword = text
        }
    }
    Field {
        visible: root.security === "enterprise"
        label: "Network certificate"
        Segmented {
            width: parent.width
            readonly property var values: ["system", "file", "none"]
            options: ["System", "From file", "Don't check"]
            current: Math.max(0, values.indexOf(root.ca))
            onSelected: i => root.ca = values[i]
        }
    }
    FileButton {
        visible: root.security === "enterprise" && root.ca === "file"
        path: root.caFile
        onPick: root.choose("Choose the network's CA certificate", "Certificates|*.pem *.crt *.cer *.der", p => root.caFile = p)
    }
    Text {
        visible: root.security === "enterprise" && root.ca === "none"
        width: parent.width
        wrapMode: Text.Wrap
        text: "Not recommended: anyone can pretend to be this network and collect your password."
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.caption }
    }
    Field {
        visible: root.security === "enterprise" && root.ca !== "none"
        label: "Domain"
        TextField {
            width: parent.width
            commitOnBlur: false
            placeholder: "e.g. radius.example.org"
            text: root.domain
            onTextChanged: root.domain = text
        }
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: "Recommended: the certificate must belong to this domain. Your organisation's setup guide names it."
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption }
        }
    }
    Field {
        visible: root.security === "enterprise" && !root.tls
        label: "Anonymous identity"
        TextField {
            width: parent.width
            commitOnBlur: false
            placeholder: "Optional, e.g. anonymous@example.org"
            text: root.anonymous
            onTextChanged: root.anonymous = text
        }
    }

    // ── What went wrong, and the buttons ────────────────────────────────
    Text {
        visible: text !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: root.problem || root.failure
        color: Theme.color.danger
        font { family: Theme.font.family; pixelSize: Theme.font.small }
    }
    Item {
        width: parent.width
        height: 30
        FormButton {
            anchors { right: joinButton.left; rightMargin: 8 }
            text: "Cancel"
            onClicked: {
                WifiJoin.dismiss();
                root.done();
            }
        }
        FormButton {
            id: joinButton
            anchors.right: parent.right
            primary: true
            enabled: !root.joining
            text: root.joining ? (WifiJoin.stage === "saving" ? "Saving…" : "Joining…") : "Join network"
            onClicked: root.submit()
        }
    }

    // A label above its control
    component Field: Column {
        property string label
        width: parent?.width ?? 0
        spacing: 4
        Text {
            text: parent.label
            color: Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.caption; weight: Font.Medium }
        }
    }
    // A chosen file's name, or "Choose file…"
    component FileButton: Squircle {
        id: fileButton
        property string path
        signal pick
        width: parent?.width ?? 0
        height: 28
        radius: Theme.radius.control - 2
        color: fileMouse.pressed ? Theme.color.fillPressed : fileMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
        Icon {
            id: fileIcon
            x: 10
            anchors.verticalCenter: parent.verticalCenter
            name: "file-text"
            size: 14
            color: Theme.color.secondary
        }
        Text {
            anchors { left: fileIcon.right; leftMargin: 8; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
            elide: Text.ElideMiddle
            text: fileButton.path ? root.fileName(fileButton.path) : "Choose file…"
            color: fileButton.path ? Theme.color.label : Theme.color.secondary
            font { family: Theme.font.family; pixelSize: Theme.font.body }
        }
        MouseArea {
            id: fileMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: fileButton.pick()
        }
    }
    // Cancel and Join network
    component FormButton: Squircle {
        id: button
        property string text
        property bool primary: false
        signal clicked
        width: buttonLabel.implicitWidth + 24
        height: 28
        radius: Theme.radius.control - 2
        opacity: enabled ? 1 : 0.6
        color: primary ? (buttonMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent) : buttonMouse.pressed ? Theme.color.fillPressed : buttonMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill
        Text {
            id: buttonLabel
            anchors.centerIn: parent
            text: button.text
            color: button.primary ? Theme.color.onAccent : Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
        MouseArea {
            id: buttonMouse
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: button.clicked()
        }
    }
}
