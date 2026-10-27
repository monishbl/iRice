import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// One dock slot: an app (pinned, running or recent), the Apps button, a
// minimized window (live thumbnail with its app's badge), Downloads or Trash.
// Hover lifts the icon on a spring; with magnification on, icons grow with
// pointer proximity (cosine falloff over ~2.5 icons). Right-click for a menu.
Item {
    id: root

    required property var item
    required property var dock
    property real restCenter: 0

    readonly property string kind: item.kind
    readonly property bool isApp: kind === "app" || kind === "recent"
    readonly property var windows: isApp ? Apps.windowsOf(item.id) : []
    readonly property bool running: windows.length > 0
    readonly property int base: dock.iconSize
    readonly property real proximity: dock.pointerX < -9999 ? 0 : Math.cos(Math.min(1, Math.abs(dock.pointerX - restCenter) / (base * 2.5)) * Math.PI / 2)
    readonly property real size: base * (1 + (dock.maxScale - 1) * proximity)
    // Hover comes from the dock's pointer tracking (it sits on top of the icons)
    readonly property real leftEdge: dock.pad + x + lead
    readonly property bool hot: dock.hovered && dock.menuFor === "" && dock.pointerLocal >= leftEdge - dock.gap / 2 && dock.pointerLocal < leftEdge + size + dock.gap / 2
    readonly property int lead: item.divider ? dock.dividerWidth : 0

    width: lead + size
    height: size
    anchors.bottom: parent?.bottom // grow upward from the pill's edge

    // Divider before a new section
    Rectangle {
        visible: root.item.divider ?? false
        x: Math.round((root.lead - 1) / 2) - root.dock.gap / 2
        anchors.bottom: parent.bottom
        anchors.bottomMargin: root.base * 0.1
        width: 1
        height: root.base * 0.8
        color: Theme.color.separator
    }

    Item {
        id: art
        x: root.lead
        width: root.size
        height: root.size
        // Hover lift and press squish, on springs
        property real lift: root.hot && root.dock.maxScale <= 1 ? root.base * 0.14 : 0
        Behavior on lift { SpringAnimation { spring: 4.5; damping: 0.28; epsilon: 0.05 } }
        y: -lift - bounce.offset
        scale: mouse.pressed ? 0.9 : 1
        Behavior on scale { SpringAnimation { spring: 6; damping: 0.35; epsilon: 0.002 } }

        // Apps button: the icon theme's app grid when it has one (styled like
        // every other icon), else a 3×3 grid of colour dots on a tile
        readonly property string launcherIcon: root.kind === "launcher" ? IconLookup.path(["view-app-grid", "applications-all", "launchpad", "start-here-kde", "start-here", "view-grid"], []) : ""
        readonly property bool launcherThemed: launcherIcon !== "" && !launcherIcon.includes("application-x-executable") && !launcherIcon.includes("default-icon") && !launcherIcon.includes("-symbolic")
        AppIcon {
            visible: root.kind === "launcher" && art.launcherThemed
            size: root.size
            // Rendered once at the largest magnified size, like the other
            // icons (sized to the live size, it re-rendered on every step of
            // the hover magnification and flickered)
            renderSize: Math.ceil(root.base * Math.max(1, root.dock.maxScale))
            mipmap: true
            source: art.launcherThemed ? art.launcherIcon : ""
        }
        Rectangle {
            visible: root.kind === "launcher" && !art.launcherThemed
            anchors.fill: parent
            anchors.margins: root.size * 0.04
            radius: width * 0.225
            color: Theme.dark ? "#2C2C31" : "#F4F4F5"
            border.width: 1
            border.color: Theme.dark ? "#36363C" : "#D4D4D8"
            Grid {
                anchors.centerIn: parent
                columns: 3
                spacing: parent.width * 0.09
                Repeater {
                    model: Theme.dark ? ["#93A3FC", "#F5886C", "#6EC272", "#E087CE", "#E7963D", "#25C2C2", "#BF93F1", "#F3829F", "#93A3FC"] : ["#6D7CDF", "#D05F43", "#409D48", "#BB5FAA", "#BE7100", "#1D9999", "#9B6BCE", "#CE597A", "#6D7CDF"]
                    Rectangle {
                        required property string modelData
                        width: root.size * 0.15
                        height: width
                        radius: width / 2
                        color: modelData
                    }
                }
            }
        }

        // Minimized window: its last frame, with the app's badge
        Rectangle {
            visible: root.kind === "minimized"
            anchors.fill: parent
            anchors.margins: root.size * 0.06
            radius: width * 0.12
            color: Theme.color.fill
            clip: true
            ScreencopyView {
                anchors.fill: parent
                captureSource: root.kind === "minimized" ? (root.item.minimized?.wayland ?? null) : null
                live: false
                constraintSize: Qt.size(root.base * 2, root.base * 2)
            }
        }

        AppIcon {
            visible: root.kind !== "launcher"
            x: root.kind === "minimized" ? parent.width * 0.5 : 0
            y: root.kind === "minimized" ? parent.height * 0.5 : 0
            size: root.kind === "minimized" ? root.size * 0.5 : root.size
            renderSize: Math.ceil((root.kind === "minimized" ? 0.5 : 1) * root.base * Math.max(1, root.dock.maxScale))
            mipmap: true
            source: root.kind === "launcher" ? "" : root.kind === "downloads" ? IconLookup.path(["folder-download", "folder-downloads", "folder_download", "folder-download-open"], ["folder", "inode-directory"]) : root.kind === "trash" ? IconLookup.path(trashFiles.count > 0 ? ["user-trash-full", "trash-full", "user-trash"] : ["user-trash", "trash-empty", "user-trash-empty"], ["edit-delete"]) : Apps.iconFor(root.item.entry ?? (root.kind === "minimized" ? (root.item.minimized?.lastIpcObject?.class ?? "") : root.item.id))
        }

        // Badge: the app's unread count, or a dot when it wants attention
        Rectangle {
            readonly property int count: root.item.entry ? Badges.countFor(root.item.id) : 0
            readonly property bool dot: count === 0 && (root.item.entry ? Badges.urgentFor(root.item.id) : false)
            visible: count > 0 || dot
            anchors { right: parent.right; top: parent.top; rightMargin: -root.size * 0.06; topMargin: -root.size * 0.06 }
            height: Math.round(Math.max(14, root.size * 0.34))
            width: dot ? height : Math.max(height, badgeText.implicitWidth + height * 0.6)
            radius: height / 2
            color: "#FF3B30"
            border.width: 1
            border.color: Qt.rgba(0, 0, 0, 0.15)
            scale: visible ? 1 : 0
            // (a NumberAnimation: it pops in as it becomes visible, when a
            // ScaleAnimator may not start and leave the badge at 0)
            Behavior on scale { NumberAnimation { duration: 200; easing.type: Easing.OutBack } }
            Text {
                id: badgeText
                anchors.centerIn: parent
                visible: !parent.dot
                text: parent.count > 99 ? "99+" : String(parent.count)
                color: "white"
                font { family: Theme.font.family; pixelSize: Math.round(parent.height * 0.62); weight: Font.DemiBold; features: { "tnum": 1 } }
            }
        }
    }

    QtObject {
        id: bounce
        property real offset: 0
    }
    SequentialAnimation {
        id: bounceAnim
        loops: 2
        NumberAnimation { target: bounce; property: "offset"; to: root.base * 0.35; duration: 260; easing.type: Easing.OutQuad }
        NumberAnimation { target: bounce; property: "offset"; to: 0; duration: 260; easing.type: Easing.InQuad }
    }

    // Running indicator
    Rectangle {
        visible: root.running
        x: root.lead + (root.size - width) / 2
        y: root.size + root.dock.pad * 0.35
        width: 4
        height: 4
        radius: 2
        color: Theme.color.label
        opacity: 0.8
    }

    // Name above the hovered icon, on frosted glass (the dock's layer is blurred)
    GlassPopupBackground {
        visible: root.hot
        x: root.lead + (root.size - width) / 2
        y: art.y - height - 10
        width: label.implicitWidth + 20
        height: 26
        radius: 13
        blurContent: false // (the compositor blurs the dock's layer already)
        Text {
            id: label
            anchors.centerIn: parent
            text: root.kind === "launcher" ? "Apps" : root.kind === "downloads" ? "Downloads" : root.kind === "trash" ? "Trash" : root.kind === "minimized" ? (root.item.minimized?.title || root.item.entry?.name || "Window") : (root.item.entry?.name ?? root.item.id)
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
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
        y: -art.lift
        width: root.size
        height: root.size + art.lift
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => {
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
            default:
                if (!root.running)
                    bounceAnim.restart();
                // Clicking the app that's already in front: App Exposé
                if (root.running && Apps.activeEntry?.id === root.item.id)
                    Panels.appExpose(root.item.id, root.dock.screen);
                else
                    Apps.activateOrLaunch(root.item);
            }
        }
    }
}
