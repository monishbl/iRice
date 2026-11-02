import QtQuick
import Quickshell
import Quickshell.Wayland
import "../lock" as Lock
import "../../common/material-symbols.js" as Symbols
import "../../common/MaterialShapes.js" as Shapes

// The login screen in the Material look (GreeterLook), as Android 16's lock
// screen: the wallpaper, the big two-line clock, and at the bottom the
// person's picture in a cookie, their name, a pill password field and the
// session as a chip; sleep, restart and shut down at the bottom right. Every
// screen shows the clock; the focused one signs in. Same keys and flow as
// the Glass one (GreeterSurface): ↑↓ user, Tab session, Enter signs in,
// F2/F3/F4 power (Enter or the same again confirms, Esc cancels). Nothing
// here reads Theme or Config: colours come from GreeterLook.m3.
PanelWindow {
    id: win

    required property var auth
    required property var appearance // GreeterLook
    property bool primary: false
    readonly property var m3: appearance.m3

    color: m3.surfaceContainerLowest
    WlrLayershell.namespace: "irice-greeter"
    WlrLayershell.layer: WlrLayer.Top
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property string family: "Google Sans Flex"
    property string powerChoice: ""
    readonly property var powerActions: [
        { key: "F2", label: "Sleep", icon: "moon", action: "suspend" },
        { key: "F3", label: "Restart", icon: "arrow-clockwise", action: "reboot" },
        { key: "F4", label: "Shut down", icon: "power", action: "poweroff" }
    ]
    function powerLabel(action) {
        const a = powerActions.find(p => p.action === action);
        return a ? a.label : action;
    }

    function nextUser(step) {
        if (auth.busy || !auth.users.length)
            return;
        auth.user = (auth.user + auth.users.length + step) % auth.users.length;
        input.text = "";
    }
    function nextSession() {
        if (!auth.busy && auth.sessions.length)
            auth.session = (auth.session + 1) % auth.sessions.length;
    }
    // Keyboard first, but everything also works by pointer: Sign in, and the
    // power actions (choosing one asks to confirm: Enter, or the same again)
    function submit() {
        if (powerChoice) {
            const action = powerChoice;
            powerChoice = "";
            if (auth.mock)
                console.log("greeter (mock):", action);
            else
                Quickshell.execDetached(["systemctl", action]);
        } else if (!auth.busy) {
            auth.login(input.text);
        }
    }
    function choosePower(action) {
        if (auth.busy)
            return;
        if (powerChoice === action)
            submit();
        else
            powerChoice = action;
        input.forceActiveFocus();
    }
    onPrimaryChanged: if (primary) input.forceActiveFocus()
    Connections {
        target: win.auth
        function onErrorChanged() {
            if (win.auth.error) {
                input.text = "";
                shakeAnim.restart();
            }
        }
        function onUserChanged() { input.text = ""; }
    }

    // A Material Symbols glyph (the greeter can't use common/Icon: it reads Theme)
    component Glyph: Text {
        property string name
        property bool fill: false
        property int size: 24
        text: Symbols.glyphs[name] ?? ""
        font.family: "iRice Symbols"
        font.pixelSize: Math.round(size * 1.2)
        font.variableAxes: { "FILL": fill ? 1 : 0, "wght": 400, "opsz": Math.max(20, Math.min(48, size * 1.2)), "GRAD": 0 }
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    // ── The wallpaper, dimmed a little so the text reads ────────────────
    Image {
        anchors.fill: parent
        visible: win.appearance.wallpaper !== ""
        source: win.appearance.wallpaper !== "" ? "file://" + win.appearance.wallpaper : ""
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(win.width, win.height)
        asynchronous: true
    }
    Rectangle {
        anchors.fill: parent
        color: Qt.alpha(win.m3.surfaceContainerLowest, win.appearance.dark ? 0.45 : 0.55)
    }

    readonly property bool split: width > height * 1.2
    readonly property color ink: appearance.dark ? m3.secondaryFixed : m3.onSecondaryFixed
    readonly property color ink2: appearance.dark ? m3.secondaryFixedDim : m3.onSecondaryFixedVariant

    SystemClock { id: wallClock; precision: SystemClock.Minutes }

    // ── Clock and date (every screen; to the left on a wide primary) ────
    Column {
        id: header
        readonly property bool atLeft: win.primary && win.split
        x: atLeft ? Math.round(win.width * 0.08) : Math.round((win.width - width) / 2)
        y: Math.round(win.height * (atLeft ? 0.14 : win.primary ? 0.08 : 0.3))
        spacing: 20
        Lock.MaterialClock {
            id: clock
            anchors.horizontalCenter: header.atLeft ? undefined : parent.horizontalCenter
            width: implicitWidth
            height: implicitHeight
            time: wallClock.date
            size: Math.round(win.primary && win.split ? Math.min(win.height * 0.27, win.width * 0.16) : Math.min(win.height * 0.22, win.width * 0.3))
            color: win.appearance.dark ? win.m3.primaryFixed : win.m3.onPrimaryFixed
            color2: win.appearance.dark ? win.m3.primaryFixedDim : win.m3.onPrimaryFixedVariant
            align: header.atLeft ? Text.AlignLeft : Text.AlignHCenter
            ticks: true
        }
        Text {
            anchors.horizontalCenter: header.atLeft ? undefined : parent.horizontalCenter
            x: header.atLeft ? 4 : 0
            text: Qt.formatDate(wallClock.date, "dddd d MMMM")
            color: win.ink
            font { family: win.family; pixelSize: 20; weight: Font.Medium }
        }
    }

    // ── Sign in (the focused screen) ────────────────────────────────────
    Column {
        id: login
        visible: win.primary
        x: Math.round((win.width - width) / 2)
        y: win.height - height - Math.round(win.height * 0.06)
        spacing: 16

        // Who: the picture (AccountsService's, else an initial) in a cookie,
        // with arrows to the others
        Row {
            id: who
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 16
            readonly property bool several: win.auth.users.length > 1
            Item {
                visible: who.several
                anchors.verticalCenter: parent.verticalCenter
                width: 48
                height: 48
                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: prevMouse.pressed ? Qt.alpha(win.m3.onSurface, 0.1) : prevMouse.containsMouse ? Qt.alpha(win.m3.onSurface, 0.08) : "transparent"
                }
                Glyph { anchors.centerIn: parent; name: "caret-left"; size: 24; color: win.ink }
                MouseArea { id: prevMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: win.nextUser(-1) }
                Accessible.role: Accessible.Button
                Accessible.name: "Previous user"
            }
            Item {
                id: avatar
                width: 96
                height: 96
                readonly property var person: win.auth.users[win.auth.user] ?? null
                Image {
                    id: avatarImage
                    anchors.fill: parent
                    source: avatar.person ? "file:///var/lib/AccountsService/icons/" + avatar.person.name : ""
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(192, 192)
                    asynchronous: true
                }
                GreeterShape {
                    anchors.fill: parent
                    shape: "cookie9"
                    color: win.m3.primaryContainer
                    source: avatarImage.status === Image.Ready ? avatarImage : null
                    spin: win.auth.busy ? 40 : 0
                    Behavior on spin { SpringAnimation { spring: 3.2; damping: 0.27; epsilon: 0.05 } }
                }
                Text {
                    anchors.centerIn: parent
                    visible: avatarImage.status !== Image.Ready
                    text: (avatar.person?.full ?? "?").charAt(0).toUpperCase()
                    color: win.m3.onPrimaryContainer
                    font.family: win.family
                    font.pixelSize: 40
                    font.variableAxes: { "wght": 600, "ROND": 100 }
                }
                MouseArea { anchors.fill: parent; enabled: who.several; cursorShape: Qt.PointingHandCursor; onClicked: win.nextUser(1) }
            }
            Item {
                visible: who.several
                anchors.verticalCenter: parent.verticalCenter
                width: 48
                height: 48
                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: nextMouse.pressed ? Qt.alpha(win.m3.onSurface, 0.1) : nextMouse.containsMouse ? Qt.alpha(win.m3.onSurface, 0.08) : "transparent"
                }
                Glyph { anchors.centerIn: parent; name: "caret-right"; size: 24; color: win.ink }
                MouseArea { id: nextMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: win.nextUser(1) }
                Accessible.role: Accessible.Button
                Accessible.name: "Next user"
            }
        }
        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 2
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: win.auth.users[win.auth.user]?.full ?? "No users"
                color: win.ink
                font { family: win.family; pixelSize: 22; weight: Font.Medium }
            }
            Text {
                readonly property var person: win.auth.users[win.auth.user] ?? null
                visible: !!person && person.full !== person.name
                anchors.horizontalCenter: parent.horizontalCenter
                text: person?.name ?? ""
                color: win.ink2
                font { family: win.family; pixelSize: 14 }
            }
        }

        // What's happening, or what went wrong and how to fix it, above the
        // field (it stays put)
        Rectangle {
            id: messageChip
            readonly property string message: win.powerChoice ? win.powerLabel(win.powerChoice) + "? Press Enter or click it again to confirm, Esc to cancel."
                : win.auth.error || (win.auth.busy ? "Signing in…" : win.auth.mock ? "Preview password: irice" : "")
            readonly property bool bad: win.auth.error !== "" && !win.powerChoice
            visible: message !== ""
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(messageRow.implicitWidth, 480) + 32
            height: messageRow.height + 20
            radius: messageText.lineCount > 1 ? 20 : height / 2
            color: win.m3.surfaceContainerHigh
            Row {
                id: messageRow
                anchors.centerIn: parent
                spacing: 8
                Glyph {
                    visible: messageChip.bad || win.powerChoice !== ""
                    name: win.powerChoice ? "power" : "warning-circle"
                    fill: true
                    size: 18
                    color: win.m3.error
                }
                Text {
                    id: messageText
                    width: Math.min(implicitWidth, 480 - 26)
                    wrapMode: Text.WordWrap
                    text: messageChip.message
                    color: messageChip.bad || win.powerChoice ? win.m3.error : win.m3.onSurfaceVariant
                    font { family: win.family; pixelSize: 14; weight: Font.Medium }
                }
            }
        }

        // The password: a filled pill, outlined while typing, the send
        // button squaring off once there's something to send
        Item {
            id: fieldBox
            anchors.horizontalCenter: parent.horizontalCenter
            width: 360
            height: 56
            property real shake: 0
            SpringAnimation {
                id: shakeAnim
                target: fieldBox
                property: "shake"
                from: 18
                to: 0
                spring: 24
                damping: 0.19
                epsilon: 0.2
            }
            Rectangle {
                x: fieldBox.shake
                width: parent.width
                height: parent.height
                radius: height / 2
                color: win.m3.surfaceContainerHigh
                border.width: input.activeFocus || win.auth.error !== "" ? 2 : 0
                border.color: win.auth.error !== "" ? win.m3.error : win.m3.primary

                TextInput {
                    id: input
                    x: 24
                    width: parent.width - 24 - 60
                    height: parent.height
                    verticalAlignment: TextInput.AlignVCenter
                    echoMode: TextInput.Password
                    passwordCharacter: "•"
                    readOnly: win.auth.busy || win.powerChoice !== ""
                    focus: win.primary
                    color: win.m3.onSurface
                    selectionColor: win.m3.primary
                    selectedTextColor: win.m3.onPrimary
                    font { family: win.family; pixelSize: 18; letterSpacing: 2 }
                    Accessible.name: "Password"
                    Component.onCompleted: if (win.primary) forceActiveFocus()
                    onAccepted: win.submit()
                    Keys.onUpPressed: win.nextUser(-1)
                    Keys.onDownPressed: win.nextUser(1)
                    Keys.onTabPressed: win.nextSession()
                    Keys.onEscapePressed: {
                        win.powerChoice = "";
                        text = "";
                    }
                    Keys.onPressed: event => {
                        const actions = { [Qt.Key_F2]: "suspend", [Qt.Key_F3]: "reboot", [Qt.Key_F4]: "poweroff" };
                        if (actions[event.key] && !win.auth.busy) {
                            win.powerChoice = actions[event.key];
                            event.accepted = true;
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: input.text === ""
                        text: "Enter password"
                        color: win.m3.onSurfaceVariant
                        font { family: win.family; pixelSize: 16 }
                    }
                }

                Item {
                    id: send
                    anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    width: 40
                    height: 40
                    readonly property bool ready: input.text !== "" || win.powerChoice !== ""
                    visible: !win.auth.busy
                    GreeterShape {
                        anchors.fill: parent
                        shape: send.ready ? "square" : "circle"
                        color: send.ready ? (sendMouse.containsMouse ? Qt.tint(win.m3.primary, Qt.alpha(win.m3.onPrimary, sendMouse.pressed ? 0.1 : 0.08)) : win.m3.primary) : Qt.alpha(win.m3.onSurface, 0.12)
                    }
                    Glyph {
                        anchors.centerIn: parent
                        name: "arrow-right"
                        size: 20
                        color: send.ready ? win.m3.onPrimary : Qt.alpha(win.m3.onSurface, 0.38)
                    }
                    MouseArea {
                        id: sendMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: win.submit()
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: "Sign in"
                    Accessible.onPressAction: win.submit()
                }
                // Signing in: Material's loading indicator, a shape morphing
                // through the loading cycle a step every 650 ms
                GreeterShape {
                    id: loading
                    anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    visible: win.auth.busy
                    width: 36
                    height: 36
                    color: win.m3.primary
                    property int step: 0
                    shape: Shapes.loading[step % Shapes.loading.length]
                    property real quarter: 0
                    spin: quarter
                    Behavior on quarter { SpringAnimation { spring: 3.2; damping: 0.27; epsilon: 0.05 } }
                    Timer {
                        interval: 650
                        repeat: true
                        running: loading.visible
                        onTriggered: {
                            loading.step++;
                            loading.quarter += 90;
                        }
                    }
                }
            }
        }

        // The session, as a chip (Tab, or click, for the next)
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: sessionRow.implicitWidth + 32
            height: 36
            radius: 12
            color: sessionMouse.containsMouse ? Qt.tint(win.m3.secondaryContainer, Qt.alpha(win.m3.onSecondaryContainer, sessionMouse.pressed ? 0.1 : 0.08)) : win.m3.secondaryContainer
            Row {
                id: sessionRow
                anchors.centerIn: parent
                spacing: 8
                Glyph { anchors.verticalCenter: parent.verticalCenter; name: "desktop"; size: 18; color: win.m3.onSecondaryContainer }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: win.auth.sessions[win.auth.session]?.name ?? "Loading…"
                    color: win.m3.onSecondaryContainer
                    font { family: win.family; pixelSize: 14; weight: Font.Medium }
                }
                Glyph { anchors.verticalCenter: parent.verticalCenter; name: "caret-up-down"; size: 16; color: win.m3.onSecondaryContainer }
            }
            MouseArea {
                id: sessionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: win.nextSession()
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Session: " + (win.auth.sessions[win.auth.session]?.name ?? "") + ". Choose the next one."
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: (win.auth.users.length > 1 ? "↑↓ user · " : "") + "Tab session · Enter to sign in"
            color: win.ink2
            font { family: win.family; pixelSize: 12 }
        }
    }

    // ── Sleep, restart, shut down (F2, F3, F4) ──────────────────────────
    Row {
        visible: win.primary
        anchors { right: parent.right; bottom: parent.bottom; margins: 32 }
        spacing: 12
        Repeater {
            model: win.powerActions
            Column {
                id: power
                required property var modelData
                readonly property bool chosen: win.powerChoice === modelData.action
                spacing: 6
                Item {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 52
                    height: 52
                    // Chosen: the container squares off in error, asking to confirm
                    GreeterShape {
                        anchors.fill: parent
                        shape: power.chosen ? "square" : "circle"
                        color: power.chosen ? win.m3.errorContainer : powerMouse.pressed ? Qt.tint(win.m3.surfaceContainerHigh, Qt.alpha(win.m3.onSurface, 0.1)) : powerMouse.containsMouse ? Qt.tint(win.m3.surfaceContainerHigh, Qt.alpha(win.m3.onSurface, 0.08)) : win.m3.surfaceContainerHigh
                    }
                    Glyph {
                        anchors.centerIn: parent
                        name: power.modelData.icon
                        size: 22
                        color: power.chosen ? win.m3.onErrorContainer : win.m3.onSurfaceVariant
                    }
                    MouseArea {
                        id: powerMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: win.choosePower(power.modelData.action)
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: power.modelData.label + " (" + power.modelData.key + ")"
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: power.modelData.label + " " + power.modelData.key
                    color: power.chosen ? win.m3.error : win.ink2
                    font { family: win.family; pixelSize: 12; weight: Font.Medium }
                }
            }
        }
    }
}
