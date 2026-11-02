import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Someone nearby wants to share something (Quick Share or LocalSend:
// Sharing.request), as AirDrop asks on iOS: a glass alert in the middle of
// the screen you're using, with who it is, what it is, the PIN their phone
// shows, and Decline / Accept. Accepted, a ring fills around them while it
// arrives; then Open and Show in Files. Hide leaves the transfer to the
// island. It doesn't block the rest of the screen.
// Material: a dialog (surfaceContainerHigh, corner 28), who it is on a shape
// that changes as it goes, a wavy bar while it arrives, filled and tonal
// buttons.
Scope {
    id: root

    readonly property var req: Sharing.request
    readonly property var item: Sharing.progressOf(req)
    readonly property real frac: item && item.total > 0 ? Math.min(1, item.done / item.total) : 0
    readonly property string state_: req?.state ?? ""
    readonly property int count: req?.files?.length ?? 0
    readonly property bool photos: count > 0 && (req?.files ?? []).every(f => String(f.mime ?? "").startsWith("image/") || /\.(jpe?g|png|heic|webp|gif)$/i.test(f.name ?? ""))
    readonly property string what: count === 0 ? (req?.what ?? "something") : count === 1 ? "“" + req.files[0].name + "”" : count + (photos ? " photos" : " files")
    // How it's arriving, shown on the phone's screen in the avatar: the same
    // Wi-Fi, a direct link (the phone's hotspot or ours) or Bluetooth
    readonly property string linkIcon: {
        const t = String(item?.transport ?? "").toLowerCase();
        return t === "" ? "" : t.includes("bluetooth") ? "bluetooth" : t.includes("direct") || t.includes("hotspot") ? "broadcast" : "wifi-high";
    }
    readonly property bool handheld: req?.type === "phone" || req?.type === "mobile" || req?.type === "tablet"
    readonly property string kindIcon: (req?.type === "phone" || req?.type === "mobile") ? "device-mobile" : req?.type === "tablet" ? "device-tablet" : req?.type === "laptop" ? "laptop" : "desktop"

    // Done: it goes after a while (Open / Show in Files are there till then)
    Timer {
        running: root.state_ === "done" || root.state_ === "failed"
        interval: 9000
        onTriggered: if (root.req) Sharing.dropRequest(root.req.id)
    }

    LazyLoader {
        active: root.req !== null

        GlassWindow {
            id: win
            name: "share-incoming"
            screen: Quickshell.screens.find(s => s.name === Hyprland.focusedMonitor?.name) ?? Quickshell.screens[0]
            implicitWidth: card.width + 2 * glass.shadowMargin
            implicitHeight: card.height + 2 * glass.shadowMargin
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            // (keys once it's clicked: it doesn't take your typing on its own)
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand

            property real shown: 0
            Component.onCompleted: shown = 1
            Behavior on shown { NumberAnimation { duration: Math.round(320 * CoolDown.motionScale); easing.type: Easing.OutBack; easing.overshoot: 1.2 } }

            Glass {
                id: glass
                anchors.fill: parent
                radius: Theme.radius.panel
                opacity: win.shown
                scale: 0.9 + 0.1 * win.shown
            }
            // (Material: a dialog's surfaceContainerHigh)
            Rectangle {
                visible: Theme.material
                anchors.fill: card
                radius: 28
                color: Theme.m3.surfaceContainerHigh ?? "transparent"
                opacity: win.shown
                scale: 0.9 + 0.1 * win.shown
            }

            Item {
                id: card
                x: glass.shadowMargin
                y: glass.shadowMargin
                width: Theme.material ? 312 : 300
                height: body.implicitHeight + 2 * body.x
                opacity: win.shown
                scale: 0.9 + 0.1 * win.shown
                focus: true
                Keys.onReturnPressed: if (root.state_ === "asking") Sharing.answer(root.req.id, true)
                Keys.onEscapePressed: root.state_ === "asking" ? Sharing.answer(root.req.id, false) : Sharing.dropRequest(root.req.id)

                Column {
                    id: body
                    x: Theme.material ? 24 : 20
                    y: Theme.material ? 24 : 22
                    width: parent.width - 2 * x
                    spacing: 6

                    // Who: a round avatar, and the ring that fills as it arrives
                    Item {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 84
                        height: 84
                        // Material: they're on a shape: a cookie while it asks,
                        // round while it arrives, sunny when it's there, a
                        // square in errorContainer if it went wrong
                        Loader {
                            anchors.centerIn: parent
                            active: Theme.material
                            sourceComponent: MaterialShape {
                                width: 76
                                height: 76
                                shape: root.state_ === "receiving" ? "circle" : root.state_ === "done" ? "sunny" : root.state_ === "failed" ? "square" : "cookie9"
                                color: root.state_ === "failed" ? Theme.m3.errorContainer : Theme.m3.primaryContainer
                            }
                        }
                        Rectangle {
                            anchors.centerIn: parent
                            width: 68
                            height: 68
                            radius: 34
                            // (Material: only what's on it; the shape is the container)
                            readonly property color ink: Theme.material ? (root.state_ === "failed" ? Theme.m3.onErrorContainer : Theme.m3.onPrimaryContainer) : Theme.color.onAccent
                            gradient: Gradient {
                                GradientStop { position: 0; color: Theme.material ? "transparent" : Qt.lighter(Theme.accent, 1.25) }
                                GradientStop { position: 1; color: Theme.material ? "transparent" : Theme.accent }
                            }
                            Icon {
                                anchors.centerIn: parent
                                visible: !root.handheld
                                name: root.kindIcon
                                fill: true
                                size: 32
                                color: parent.ink
                            }
                            // A phone (or tablet), its screen showing the link
                            Rectangle {
                                visible: root.handheld
                                anchors.centerIn: parent
                                width: root.req?.type === "tablet" ? 34 : 26
                                height: 42
                                radius: 7
                                color: "transparent"
                                border.width: 2.5
                                border.color: parent.ink
                                Rectangle {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: 4
                                    width: 8
                                    height: 2.5
                                    radius: 1.25
                                    color: parent.parent.ink
                                }
                                Icon {
                                    anchors.centerIn: parent
                                    anchors.verticalCenterOffset: 2
                                    visible: root.linkIcon !== ""
                                    name: root.linkIcon
                                    fill: true
                                    size: 14
                                    color: parent.parent.ink
                                }
                            }
                        }
                        Canvas {
                            id: ring
                            anchors.fill: parent
                            visible: root.state_ === "receiving" && !Theme.material
                            readonly property real f: root.frac
                            onFChanged: requestPaint()
                            onVisibleChanged: requestPaint()
                            onPaint: {
                                const ctx = getContext("2d");
                                ctx.reset();
                                ctx.lineWidth = 4;
                                ctx.lineCap = "round";
                                ctx.strokeStyle = Theme.color.fill;
                                ctx.beginPath();
                                ctx.arc(width / 2, height / 2, width / 2 - 3, 0, 2 * Math.PI);
                                ctx.stroke();
                                ctx.strokeStyle = Theme.accent;
                                ctx.beginPath();
                                ctx.arc(width / 2, height / 2, width / 2 - 3, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * f);
                                ctx.stroke();
                            }
                        }
                        // Arrived: a check; went wrong: a mark
                        Rectangle {
                            visible: root.state_ === "done" || root.state_ === "failed"
                            anchors { right: parent.right; bottom: parent.bottom; rightMargin: 6; bottomMargin: 6 }
                            width: 26
                            height: 26
                            radius: 13
                            // (Material: done in primary, failed in error)
                            color: Theme.material ? (root.state_ === "done" ? Theme.m3.primary : Theme.m3.error) : root.state_ === "done" ? Theme.color.success : Theme.color.danger
                            border.width: 2
                            border.color: Theme.material ? Theme.m3.surfaceContainerHigh : Theme.glass.solid
                            Icon {
                                anchors.centerIn: parent
                                name: root.state_ === "done" ? "check" : "x"
                                size: 14
                                color: Theme.material ? (root.state_ === "done" ? Theme.m3.onPrimary : Theme.m3.onError) : "white"
                            }
                        }
                    }

                    // Material: how far it's got, a wavy bar
                    Loader {
                        anchors.horizontalCenter: parent.horizontalCenter
                        active: Theme.material && root.state_ === "receiving"
                        visible: active
                        sourceComponent: WavyProgress {
                            width: 160
                            value: root.item?.waiting ? -1 : root.frac
                        }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        text: root.req?.from ?? ""
                        elide: Text.ElideRight
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        text: root.state_ === "asking" ? "would like to share " + root.what + (root.req?.total > 0 ? " (" + Transfers.size(root.req.total) + ")" : "") + "."
                            : root.state_ === "receiving" ? (root.item?.waiting ? "Starting…" : "Receiving " + root.what + "… " + Math.round(100 * root.frac) + "%")
                            : root.state_ === "done" ? (root.count === 1 ? root.what + " is in " : root.what + " are in ") + (Config.options.sharing.folder ? Config.options.sharing.folder.split("/").pop() : "Downloads") + "."
                            : (root.req?.message || "It stopped before everything arrived.")
                        color: Theme.color.secondary
                        font { family: Theme.font.family; pixelSize: Theme.font.body }
                    }
                    // How far, in bytes (the link: on the phone's screen above)
                    Text {
                        visible: root.state_ === "receiving" && !!root.item && !root.item.waiting
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        text: root.item ? Transfers.size(root.item.done) + " of " + Transfers.size(root.item.total) : ""
                        color: Theme.color.tertiary
                        font { family: Theme.font.family; pixelSize: Theme.font.small; features: { "tnum": 1 } }
                    }
                    // The PIN their phone shows (Quick Share), to know it's them
                    Rectangle {
                        visible: root.state_ === "asking" && (root.req?.pin ?? "") !== ""
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: pinText.implicitWidth + (Theme.material ? 24 : 20)
                        // (Material: an outlined chip, 32 high, corner 8)
                        height: Theme.material ? 32 : 24
                        radius: Theme.material ? 8 : 12
                        color: Theme.material ? "transparent" : Theme.color.fill
                        border.width: Theme.material ? 1 : 0
                        border.color: Theme.color.separator
                        Text {
                            id: pinText
                            anchors.centerIn: parent
                            text: "PIN " + (root.req?.pin ?? "")
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium; features: { "tnum": 1 } }
                        }
                    }

                    Item { width: 1; height: 8 }

                    // iOS alert buttons: side by side, the one to do on the right
                    Row {
                        width: parent.width
                        spacing: 10
                        readonly property var buttons: root.state_ === "asking" ? [
                                { label: "Decline", primary: false, act: () => Sharing.answer(root.req.id, false) },
                                { label: "Accept", primary: true, act: () => Sharing.answer(root.req.id, true) }
                            ] : root.state_ === "receiving" ? [
                                { label: "Cancel", primary: false, act: () => Sharing.cancelRequest(root.req.id) },
                                { label: "Hide", primary: false, act: () => Sharing.updateRequest(root.req.id, { hidden: true }) }
                            ] : root.state_ === "done" ? (root.req.paths.length === 1 ? [
                                { label: "Show in Files", primary: false, act: () => { Files.reveal(root.req.paths[0]); Sharing.dropRequest(root.req.id); } },
                                { label: "Open", primary: true, act: () => { Launch.open("file://" + root.req.paths[0]); Sharing.dropRequest(root.req.id); } }
                            ] : [
                                { label: "Done", primary: false, act: () => Sharing.dropRequest(root.req.id) },
                                { label: "Show in Files", primary: true, act: () => { Files.reveal(root.req.paths[0] ?? ""); Sharing.dropRequest(root.req.id); } }
                            ]) : [
                                { label: "OK", primary: true, act: () => Sharing.dropRequest(root.req.id) }
                            ]
                        Repeater {
                            model: parent.buttons
                            Squircle {
                                id: btn
                                required property var modelData
                                width: (parent.width - 10 * (parent.buttons.length - 1)) / parent.buttons.length
                                height: 40
                                // Material: filled primary for the one to do, tonal
                                // (secondaryContainer) for the rest, squaring off
                                // while pressed
                                radius: Theme.material && btnMouse.pressed ? 12 : 20
                                Behavior on radius {
                                    enabled: Theme.material
                                    SpringAnimation { spring: Theme.motion.springFast.spring; damping: Theme.motion.springFast.damping; epsilon: 0.25 }
                                }
                                readonly property color ink: Theme.material ? (modelData.primary ? Theme.m3.onPrimary : Theme.m3.onSecondaryContainer) : modelData.primary ? Theme.color.onAccent : Theme.color.label
                                color: Theme.material ? Qt.tint(modelData.primary ? Theme.m3.primary : Theme.m3.secondaryContainer, Qt.alpha(ink, btnMouse.pressed ? 0.1 : btnMouse.containsMouse ? 0.08 : 0))
                                    : modelData.primary ? (btnMouse.pressed ? Qt.darker(Theme.accent, 1.15) : Theme.accent)
                                    : (btnMouse.pressed ? Theme.color.fillPressed : btnMouse.containsMouse ? Theme.color.fillHover : Theme.color.fill)
                                scale: btnMouse.pressed && !Theme.material ? 0.97 : 1
                                Behavior on scale { NumberAnimation { duration: 100 } }
                                activeFocusOnTab: true
                                Keys.onPressed: event => {
                                    if ((event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && !event.isAutoRepeat) {
                                        btn.modelData.act();
                                        event.accepted = true;
                                    }
                                }
                                Accessible.role: Accessible.Button
                                Accessible.name: modelData.label
                                Accessible.onPressAction: btn.modelData.act()
                                FocusRing { radius: 24 }
                                Text {
                                    anchors.centerIn: parent
                                    text: btn.modelData.label
                                    color: btn.ink
                                    font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                                }
                                MouseArea {
                                    id: btnMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: btn.modelData.act()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
