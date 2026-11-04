import QtQuick
import Quickshell
import qs.common
import qs.modules.lock as LockUi

// The Material login screen, as the Material lock screen (Android 17's):
// your wallpaper, the lock's clock in the style you chose (its colours from
// your palette: greeter-start.sh gives the greeter a config.json of them),
// the date under it; at the bottom one account pill, your picture in a
// cookie and your name, that springs open into the password field on a key
// or a mouse move (the key typed is kept), the fingerprint glyph at its
// end while a reader listens; 10 s untouched with nothing typed closes it.
// Up and Down choose the person, Tab the session (the chip in the corner,
// beside Sleep, Restart and Shut down, each asking to confirm).
// The window and the logging in: GreeterWindow. (Its own file, not a
// branch in Glass's: the Glass greeter stays clear of Material's common
// files, and a Material screen that can't load fails the start, so
// greeter-start.sh falls back to the text one.)
GreeterWindow {
    id: win
    passwordField: input
    nameField: nameInput
    onRefused: shakeAnim.restart()
    readonly property var m3: Theme.m3
    readonly property var cfg: Config.options.lock
    // (the weather face would go online for it: the dynamic one instead)
    readonly property string face: (cfg.clockFace ?? "dynamic") === "weather" ? "dynamic" : (cfg.clockFace ?? "dynamic")

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // The pill opens on a key or a mouse move and closes 10 s after the
    // last one, unless something's typed (or being checked, or wrong)
    property bool typing: false
    readonly property bool open: typing || win.auth.other || input.text !== "" || win.auth.busy || win.auth.error !== "" || win.powerChoice !== ""
    property real openT: open ? 1 : 0
    Behavior on openT {
        MaterialSpring { epsilon: 0.002 }
    }
    function wake() {
        typing = true;
        idle.restart();
    }
    Timer {
        id: idle
        interval: 10000
        onTriggered: win.typing = false
    }

    // No wallpaper: a clean background in Material's colours (the lock
    // screen, once you're in, keeps your wallpaper)
    Rectangle {
        anchors.fill: parent
        color: Qt.alpha(Theme.dark ? win.m3.surfaceContainerLowest : win.m3.surfaceContainer, 1)
    }
    HoverHandler {
        // (the first position isn't a move)
        property point last: Qt.point(-1, -1)
        onPointChanged: {
            const p = point.position;
            if (last.x >= 0 && (Math.abs(p.x - last.x) > 2 || Math.abs(p.y - last.y) > 2))
                win.wake();
            last = p;
        }
    }

    // The clock and the date
    Column {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: Math.round(win.height * 0.18) }
        spacing: 20
        LockUi.LockClock {
            anchors.horizontalCenter: parent.horizontalCenter
            width: implicitWidth
            height: implicitHeight
            face: win.face
            time: clock.date
            size: Math.round(Math.min(win.height * 0.15, win.width * 0.2))
            weight: win.cfg.clockWeight ?? 640
            widthAxis: win.cfg.clockWidth ?? 112
            round: win.cfg.clockRound ?? true
            color: Theme.dark ? win.m3.primaryFixed : win.m3.primary
            color2: Theme.dark ? win.m3.primaryFixedDim : win.m3.onPrimaryFixedVariant
            color3: Theme.dark ? win.m3.tertiaryFixed : win.m3.tertiary
        }
        Text {
            // (the dynamic face shows the date itself with 24-hour time)
            visible: !((win.cfg.clockFace ?? "") === "dynamic" && TimeFormat.h24)
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: Theme.dark ? win.m3.secondaryFixed : win.m3.onSurfaceVariant
            font { family: Theme.font.family; pixelSize: 18; weight: Font.Medium }
        }
    }

    // ── The account pill ────────────────────────────────────────────────
    Column {
        visible: win.primary
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: Math.round(win.height * 0.06) }
        spacing: 12

        // What went wrong, a power choice to confirm, or the reader's words
        Rectangle {
            readonly property string say: win.powerChoice ? ({ suspend: "Sleep", reboot: "Restart", poweroff: "Shut down" })[win.powerChoice] + ": press Enter or click it again"
                : win.auth.error || win.auth.message || (win.auth.mock ? "Preview: the password is tessera. Esc closes it." : "")
            visible: say !== ""
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(sayText.implicitWidth, 440) + 32
            height: sayText.height + 20
            radius: sayText.lineCount > 1 ? 20 : height / 2
            color: win.m3.surfaceContainerHigh
            Text {
                id: sayText
                anchors.centerIn: parent
                width: Math.min(implicitWidth, 440)
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                text: parent.say
                color: win.auth.error || win.powerChoice ? win.m3.error : win.m3.onSurface
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
        }

        // The name, above the open pill (it leaves the pill as it opens): a
        // click types another
        GreeterNameLabel {
            win: win
            anchors.horizontalCenter: parent.horizontalCenter
            visible: !win.auth.other
            opacity: win.openT
            enabled: win.openT > 0.5
            text: win.person?.full ?? ""
            textColor: win.m3.onSurface
            font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
            hoverFill: win.m3.surfaceContainerHigh
        }
        // A typed username (Switch user, or a click on the name), in its own pill
        Rectangle {
            visible: win.auth.other
            anchors.horizontalCenter: parent.horizontalCenter
            width: 360
            height: 56
            radius: height / 2
            color: win.m3.surfaceContainerHigh
            border.width: nameInput.activeFocus ? 2 : 0
            border.color: win.m3.primary
            GreeterNameInput {
                id: nameInput
                win: win
                x: 24
                width: parent.width - 48
                color: win.m3.onSurface
                font { family: Theme.font.family; pixelSize: 18 }
                placeholderColor: win.m3.onSurfaceVariant
                placeholderSize: 16
                selectionColor: win.m3.primary
                selectedTextColor: win.m3.onPrimary
                onTextChanged: win.wake()
            }
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8
            // (as wide as Switch user, so the pill stays centred)
            Item { width: 44; height: 44 }
            Round { visible: win.auth.users.length > 1 && !win.auth.other; icon: "caret-left"; tip: "Previous person"; onClicked: win.nextUser(-1) }
            Item {
                id: fieldBox
                readonly property bool fingered: win.auth.finger !== ""
                readonly property real restWidth: 8 + 40 + 12 + Math.min(nameText.implicitWidth, 240) + (fingered ? 12 + 40 + 8 : 24)
                width: Math.round(restWidth + (360 - restWidth) * win.openT)
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
                    id: pill
                    x: fieldBox.shake
                    width: parent.width
                    height: parent.height
                    radius: height / 2
                    color: win.m3.surfaceContainerHigh
                    border.width: win.open && (input.activeFocus || win.auth.error !== "") ? 2 : 0
                    border.color: win.auth.error !== "" ? win.m3.error : win.m3.primary

                    // Your picture in a cookie (AccountsService's)
                    Item {
                        id: avatar
                        x: 8
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        Image {
                            id: avatarImage
                            anchors.fill: parent
                            source: win.auth.avatar(win.pictureName)
                            fillMode: Image.PreserveAspectCrop
                            sourceSize: Qt.size(120, 120)
                            asynchronous: true
                        }
                        MaterialShape {
                            anchors.fill: parent
                            shape: "cookie9"
                            color: win.m3.primaryContainer
                            source: avatarImage.status === Image.Ready ? avatarImage : null
                            spin: win.auth.busy ? 40 : 0
                            Behavior on spin { SpringAnimation { spring: 3.2; damping: 0.27; epsilon: 0.05 } }
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: avatarImage.status !== Image.Ready
                            name: "user"
                            fill: true
                            size: 18
                            color: win.m3.onPrimaryContainer
                        }
                    }
                    Text {
                        id: nameText
                        x: avatar.x + avatar.width + 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, 240)
                        elide: Text.ElideRight
                        text: win.person?.full ?? ""
                        color: win.m3.onSurface
                        opacity: 1 - win.openT
                        visible: opacity > 0.01
                        font { family: Theme.font.family; pixelSize: 16; weight: Font.Medium }
                    }
                    TextInput {
                        id: input
                        x: avatar.x + avatar.width + 12
                        width: Math.max(0, send.x - x - 8)
                        height: parent.height
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: TextInput.Password
                        passwordCharacter: "•"
                        readOnly: win.auth.busy || win.powerChoice !== ""
                        focus: win.primary
                        clip: true
                        opacity: win.openT
                        color: win.m3.onSurface
                        selectionColor: win.m3.primary
                        selectedTextColor: win.m3.onPrimary
                        font { family: Theme.font.family; pixelSize: 18; letterSpacing: 2 }
                        Component.onCompleted: if (win.primary) forceActiveFocus()
                        onTextChanged: win.wake()
                        onAccepted: win.submit()
                        Keys.onUpPressed: win.nextUser(-1)
                        Keys.onDownPressed: win.nextUser(1)
                        Keys.onTabPressed: win.nextSession()
                        Keys.onEscapePressed: win.cancel()
                        Keys.onPressed: e => {
                            win.wake();
                            e.accepted = false;
                        }
                        Accessible.name: "Password"
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: input.text === ""
                            text: win.auth.busy ? "Logging in…" : "Enter password"
                            color: win.m3.onSurfaceVariant
                            font { family: Theme.font.family; pixelSize: 16 }
                        }
                    }
                    // Send: a circle, filled once there's something to send
                    Item {
                        id: send
                        x: fingerSlot.x - (fieldBox.fingered ? 4 : 0) - width
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        readonly property bool ready: input.text !== ""
                        visible: win.openT > 0.5 && !win.auth.busy
                        MaterialShape {
                            anchors.fill: parent
                            shape: "circle"
                            color: send.ready ? win.m3.primary : Qt.alpha(win.m3.onSurface, 0.12)
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: "arrow-right"
                            size: 20
                            color: send.ready ? win.m3.onPrimary : Qt.alpha(win.m3.onSurface, 0.38)
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: send.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: win.submit()
                        }
                        Accessible.role: Accessible.Button
                        Accessible.name: "Log in"
                        Accessible.onPressAction: win.submit()
                    }
                    Spinner {
                        x: send.x
                        anchors.verticalCenter: parent.verticalCenter
                        visible: win.auth.busy
                        size: 40
                    }
                    // The fingerprint, while a reader listens
                    Item {
                        id: fingerSlot
                        x: pill.width - 8 - width
                        anchors.verticalCenter: parent.verticalCenter
                        width: fieldBox.fingered ? 40 : 0
                        height: 40
                        MaterialShape {
                            anchors.fill: parent
                            visible: fieldBox.fingered
                            shape: "circle"
                            color: win.auth.finger === "miss" ? win.m3.errorContainer : win.m3.secondaryContainer
                        }
                        Icon {
                            anchors.centerIn: parent
                            visible: fieldBox.fingered
                            name: "fingerprint"
                            size: 22
                            color: win.auth.finger === "miss" ? win.m3.onErrorContainer : win.m3.primary
                        }
                        Accessible.role: Accessible.StaticText
                        Accessible.name: win.auth.message || "Fingerprint"
                    }
                    // A click on the closed pill opens it; on the name, types
                    // another
                    MouseArea {
                        anchors.fill: parent
                        visible: !win.open
                        cursorShape: Qt.IBeamCursor
                        onClicked: m => {
                            if (m.x >= nameText.x && m.x <= nameText.x + nameText.width) {
                                win.editName();
                                return;
                            }
                            win.wake();
                            input.forceActiveFocus();
                        }
                    }
                }
            }
            Round { visible: win.auth.users.length > 1 && !win.auth.other; icon: "caret-right"; tip: "Next person"; onClicked: win.nextUser(1) }
            Round { icon: win.auth.other ? "x" : "user-switch"; tip: win.auth.other ? "Back to the list" : "Switch user"; onClicked: win.auth.other ? win.cancelName() : win.editName() }
        }
    }

    // The session, and the power buttons, in the corner
    Row {
        visible: win.primary
        anchors { right: parent.right; bottom: parent.bottom; margins: 24 }
        spacing: 8
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: sessionText.implicitWidth + 32
            height: 44
            radius: 22
            color: sessionMouse.containsMouse ? Qt.tint(win.m3.surfaceContainerHigh, Qt.alpha(win.m3.onSurface, 0.08)) : win.m3.surfaceContainerHigh
            Text {
                id: sessionText
                anchors.centerIn: parent
                text: win.auth.sessions[win.auth.session]?.name ?? ""
                color: win.m3.onSurface
                font { family: Theme.font.family; pixelSize: 14; weight: Font.Medium }
            }
            MouseArea {
                id: sessionMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: win.nextSession()
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Session: " + sessionText.text
        }
        Round { icon: "moon"; tip: "Sleep"; danger: win.powerChoice === "suspend"; onClicked: win.choosePower("suspend") }
        Round { icon: "arrow-clockwise"; tip: "Restart"; danger: win.powerChoice === "reboot"; onClicked: win.choosePower("reboot") }
        Round { icon: "power"; tip: "Shut down"; danger: win.powerChoice === "poweroff"; onClicked: win.choosePower("poweroff") }
    }

    // A round tonal button (squares off a little while pressed)
    component Round: Item {
        id: round
        property string icon
        property string tip
        property bool danger: false
        signal clicked
        width: 44
        height: 44
        Rectangle {
            anchors.fill: parent
            radius: roundMouse.pressed ? 14 : 22
            Behavior on radius { MaterialSpring { speed: "fast"; epsilon: 0.2 } }
            color: round.danger ? win.m3.errorContainer : roundMouse.containsMouse ? Qt.tint(win.m3.surfaceContainerHigh, Qt.alpha(win.m3.onSurface, 0.08)) : win.m3.surfaceContainerHigh
        }
        Icon {
            anchors.centerIn: parent
            name: round.icon
            size: 20
            color: round.danger ? win.m3.onErrorContainer : win.m3.onSurfaceVariant
        }
        MouseArea {
            id: roundMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: round.clicked()
        }
        Accessible.role: Accessible.Button
        Accessible.name: tip
        Accessible.onPressAction: round.clicked()
    }
}
