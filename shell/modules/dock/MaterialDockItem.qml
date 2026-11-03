import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// One slot of the Material dock: an app (pinned, running or recent), the
// Apps button (a tonal circle), a minimized window (its last frame with the
// app's badge), the phone, Downloads or Trash. Hover and press draw
// Material's state layer in a circle behind the icon; running apps get a
// short dash under them. Right-click for the same menu as the Glass dock.
Item {
    id: root

    required property var item
    required property var dock

    readonly property string kind: item.kind
    readonly property bool isApp: kind === "app" || kind === "recent"
    readonly property var windows: isApp ? Apps.windowsOf(item.id) : []
    readonly property bool running: kind === "phone" ? Phone.mirroring(item.phone?.serial ?? "") : windows.length > 0
    readonly property bool focused: isApp && running && Apps.activeEntry?.id === item.id
    // (the phone as it is now: its battery changes after it came)
    readonly property var livePhone: kind === "phone" ? (Phone.devices.find(d => d.serial === item.phone?.serial) ?? item.phone) : null
    readonly property int slot: dock.iconSize
    readonly property int lead: item.divider ? dock.dividerWidth : 0
    // Hover comes from the dock's pointer tracking (it sits on top of the icons)
    readonly property real leftEdge: dock.pad + x + lead
    readonly property bool hot: dock.hovered && dock.menuFor === "" && dock.pointerLocal >= leftEdge - dock.gap / 2 && dock.pointerLocal < leftEdge + slot + dock.gap / 2
    readonly property bool pressed: mouse.pressed
    readonly property string name: kind === "launcher" ? "Apps" : kind === "phone" ? (livePhone?.model || "Phone") + (livePhone?.battery !== undefined ? " · " + livePhone.battery + "%" : "") : kind === "downloads" ? "Downloads" : kind === "trash" ? "Trash" : kind === "minimized" ? (item.minimized?.title || item.entry?.name || "Window") : (item.entry?.name ?? item.id)

    width: lead + slot
    height: slot
    Accessible.role: Accessible.Button
    Accessible.name: name

    // Divider before a new section: a short vertical line
    Rectangle {
        visible: root.item.divider ?? false
        x: Math.round((root.lead - 1) / 2 - root.dock.gap / 2)
        anchors.verticalCenter: parent.verticalCenter
        width: 1
        height: Math.round(root.slot / 2)
        color: Theme.m3.outlineVariant
    }

    Item {
        id: art
        x: root.lead
        y: -bounce.offset
        width: root.slot
        height: root.slot

        // State layer: a circle a little larger than the icon, onSurface at
        // 8% on hover and 10% pressed (the Apps button tints its own circle)
        Rectangle {
            visible: root.kind !== "launcher"
            anchors.centerIn: parent
            width: root.slot + 4
            height: width
            radius: width / 2
            color: Qt.alpha(Theme.m3.onSurface, root.pressed ? 0.1 : root.hot ? 0.08 : 0)
            Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
        }

        // The icon, squeezed a little while pressed (on the fast spatial spring)
        Item {
            id: glyph
            anchors.centerIn: parent
            width: root.dock.glyph
            height: root.dock.glyph
            transform: pressFx2.effects
            ElasticControl { id: pressFx2; pressed: root.pressed }
            // (Material: grows on press: the ElasticControl above)
            Behavior on scale {
                SpringAnimation {
                    spring: Theme.motion.springFast.spring
                    damping: Theme.motion.springFast.damping
                    epsilon: 0.002
                }
            }

            // Apps button: a tonal circle with the apps symbol, squaring off
            // while pressed like Material's buttons
            Rectangle {
                visible: root.kind === "launcher"
                anchors.fill: parent
                transform: pressFx1.effects
                ElasticControl { id: pressFx1; pressed: root.pressed }
                radius: root.pressed ? 16 : width / 2
                Behavior on radius { NumberAnimation { duration: Theme.motion.hover; easing.type: Easing.OutCubic } }
                color: Qt.tint(Theme.m3.secondaryContainer, Qt.alpha(Theme.m3.onSecondaryContainer, root.pressed ? 0.1 : root.hot ? 0.08 : 0))
                Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
                Icon {
                    anchors.centerIn: parent
                    name: "squares-four"
                    fill: true
                    size: Math.round(parent.width * 0.45)
                    color: Theme.m3.onSecondaryContainer
                }
            }

            // Minimized window: its last frame, with the app's badge
            Rectangle {
                visible: root.kind === "minimized"
                anchors.fill: parent
                anchors.margins: 2
                radius: Theme.radius.item
                color: Theme.m3.surfaceContainerHighest
                clip: true
                ScreencopyView {
                    anchors.fill: parent
                    captureSource: root.kind === "minimized" ? (root.item.minimized?.wayland ?? null) : null
                    live: false
                    constraintSize: Qt.size(root.slot * 2, root.slot * 2)
                }
            }

            AppIcon {
                visible: root.kind !== "launcher"
                x: root.kind === "minimized" ? parent.width * 0.5 : 0
                y: root.kind === "minimized" ? parent.height * 0.5 : 0
                size: root.kind === "minimized" ? parent.width * 0.5 : parent.width
                renderSize: Math.ceil(size)
                mipmap: true
                source: root.kind === "launcher" ? "" : root.kind === "downloads" ? IconLookup.path(["folder-download", "folder-downloads", "folder_download", "folder-download-open"], ["folder", "inode-directory"]) : root.kind === "trash" ? IconLookup.path(trashFiles.count > 0 ? ["user-trash-full", "trash-full", "user-trash"] : ["user-trash", "trash-empty", "user-trash-empty"], ["edit-delete"])
                    : root.kind === "phone" ? IconLookup.path(["phone", "smartphone", "phone-symbolic", "pda"], ["multimedia-player"]) : Apps.iconFor(root.item.entry ?? (root.kind === "minimized" ? (root.item.minimized?.lastIpcObject?.class ?? "") : root.item.id))
            }

            // Badge: Material's large badge (error, the count in onError), or
            // its small dot when the app wants attention
            Rectangle {
                readonly property int count: root.item.entry ? Badges.countFor(root.item.id) : 0
                readonly property bool dot: count === 0 && (root.item.entry ? Badges.urgentFor(root.item.id) : false)
                visible: count > 0 || dot
                anchors { right: parent.right; top: parent.top; rightMargin: dot ? 0 : -4; topMargin: dot ? 0 : -2 }
                height: dot ? 8 : 16
                width: dot ? height : Math.max(height, badgeText.implicitWidth + 8)
                radius: height / 2
                color: Theme.m3.error
                scale: visible ? 1 : 0
                Behavior on scale {
                    SpringAnimation {
                        spring: Theme.motion.springFast.spring
                        damping: Theme.motion.springFast.damping
                        epsilon: 0.005
                    }
                }
                Text {
                    id: badgeText
                    anchors.centerIn: parent
                    visible: !parent.dot
                    text: parent.count > 999 ? "999+" : String(parent.count)
                    color: Theme.m3.onError
                    font { family: Theme.font.family; pixelSize: 11; weight: Font.Medium; features: { "tnum": 1 } }
                }
            }
        }
    }

    QtObject {
        id: bounce
        property real offset: 0
    }
    // An app asking for attention (urgent) hops twice
    readonly property bool wantsAttention: root.isApp && root.item.entry ? Badges.urgentFor(root.item.id) : false
    onWantsAttentionChanged: if (wantsAttention) bounceAnim.restart()
    SequentialAnimation {
        id: bounceAnim
        loops: 2
        NumberAnimation { target: bounce; property: "offset"; to: root.slot * 0.3; duration: 220; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized }
        NumberAnimation { target: bounce; property: "offset"; to: 0; duration: 260; easing.type: Easing.OutBounce }
    }

    // Running: a short dash under the icon, longer and primary for the app
    // in front
    Rectangle {
        visible: root.running
        x: root.lead + (root.slot - width) / 2
        y: root.slot + Math.round((root.dock.pad - height) / 2)
        width: root.focused ? 16 : 12
        height: 2
        radius: 1
        color: root.focused ? Theme.m3.primary : Theme.m3.onSurfaceVariant
        Behavior on width {
            SpringAnimation {
                spring: Theme.motion.springFast.spring
                damping: Theme.motion.springFast.damping
                epsilon: 0.05
            }
        }
        Behavior on color { ColorAnimation { duration: Theme.motion.hover } }
    }

    // Plain tooltip above the pill: inverseSurface, corner 4, bodySmall
    Rectangle {
        id: tip
        readonly property bool shown: root.hot && root.dock.tipsWarm && root.dock.menuFor === "" && !root.dock.stackOpen
        visible: opacity > 0
        opacity: shown ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: tip.shown ? 150 : 100 } }
        x: root.lead + (root.slot - width) / 2
        y: -root.dock.pad - 8 - height + (shown ? 0 : 4)
        Behavior on y { NumberAnimation { duration: 150; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
        width: Math.min(280, tipText.implicitWidth + 16)
        height: 24
        radius: 4
        color: Theme.m3.inverseSurface
        Text {
            id: tipText
            anchors.centerIn: parent
            width: Math.min(implicitWidth, 264)
            elide: Text.ElideRight
            text: root.name
            color: Theme.m3.inverseOnSurface
            font { family: Theme.font.family; pixelSize: 12; letterSpacing: 0.4 }
        }
    }

    // Trash: full or empty. Only the Trash item has the model (an empty
    // folder makes FolderListModel list and watch the working folder)
    Loader {
        id: trashLoader
        active: root.kind === "trash"
        sourceComponent: FolderListModel {
            folder: "file://" + (Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share") + "/Trash/files"
            showHidden: true
            showDirsFirst: false
        }
    }
    readonly property QtObject trashFiles: QtObject {
        readonly property int count: trashLoader.item?.count ?? 0
    }

    MouseArea {
        id: mouse
        x: root.lead
        width: root.slot
        height: root.slot
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => {
            if (m.button === Qt.RightButton && root.kind === "phone") {
                Panels.openSettings("phone");
                return;
            }
            if (m.button === Qt.RightButton) {
                root.dock.menuItem = Object.assign({ trashCount: trashFiles.count, running: root.running }, root.item);
                root.dock.menuX = mapToItem(null, width / 2, 0).x;
                root.dock.menuFor = root.item.id;
                return;
            }
            switch (root.kind) {
            case "launcher":
                Panels.toggle("launchpad", root.dock.screen);
                break;
            case "minimized":
                Apps.restore(root.item.minimized);
                break;
            case "downloads":
                root.dock.stackX = mapToItem(null, width / 2, 0).x;
                root.dock.stackOpen = !root.dock.stackOpen;
                break;
            case "trash":
                Files.openTrash();
                break;
            case "phone":
                Phone.mirror(root.item.phone.serial);
                break;
            default:
                if (!root.running)
                    bounceAnim.restart();
                // Clicking the app that's already in front: App Exposé (only
                // when some of its windows are in sight; else they come back)
                if (root.running && Apps.activeEntry?.id === root.item.id && Apps.visibleWindowsOf(root.item.id).length > 0)
                    Panels.appExpose(root.item.id, root.dock.screen);
                else
                    Apps.activateOrLaunch(root.item);
            }
        }
    }
}
