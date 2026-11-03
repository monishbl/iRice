import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import qs.common
import qs.services

// One dock slot's state and actions, for both looks' icons (DockItem,
// MaterialDockItem draw it): an app (pinned, running or recent), the Apps
// button, a minimized window, the phone, Downloads or Trash. A click
// activates it, a right-click opens its menu; `bounce` asks the icon to hop
// (an app launching, or one asking for attention).
Item {
    id: root

    required property var item
    required property var dock

    readonly property string kind: item.kind
    readonly property bool isApp: kind === "app" || kind === "recent"
    readonly property var windows: isApp ? Apps.windowsOf(item.id) : []
    readonly property bool running: kind === "phone" ? Phone.mirroring(item.phone?.serial ?? "") : windows.length > 0
    // (the phone as it is now: its battery changes after it came)
    readonly property var livePhone: kind === "phone" ? (Phone.devices.find(d => d.serial === item.phone?.serial) ?? item.phone) : null
    readonly property int lead: item.divider ? dock.dividerWidth : 0
    readonly property string name: kind === "launcher" ? "Apps" : kind === "phone" ? (livePhone?.model || "Phone") + (livePhone?.battery !== undefined ? " · " + livePhone.battery + "%" : "") : kind === "downloads" ? "Downloads" : kind === "trash" ? "Trash" : kind === "minimized" ? (item.minimized?.title || item.entry?.name || "Window") : (item.entry?.name ?? item.id)

    signal bounce
    // An app asking for attention (urgent) hops
    readonly property bool wantsAttention: isApp && item.entry ? Badges.urgentFor(item.id) : false
    onWantsAttentionChanged: if (wantsAttention) bounce()

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

    // A click (`at`: the screen x the menu or the stack points at)
    function activate(button, at) {
        if (button === Qt.RightButton && kind === "phone") {
            Panels.openSettings("phone");
            return;
        }
        if (button === Qt.RightButton) {
            dock.menuItem = Object.assign({ trashCount: trashFiles.count, running: running }, item);
            dock.menuX = at;
            dock.menuFor = item.id;
            return;
        }
        switch (kind) {
        case "launcher":
            Panels.toggle("launchpad", dock.screen);
            break;
        case "minimized":
            Apps.restore(item.minimized);
            break;
        case "downloads":
            dock.stackX = at;
            dock.stackOpen = !dock.stackOpen;
            break;
        case "trash":
            Files.openTrash();
            break;
        case "phone":
            Phone.mirror(item.phone.serial);
            break;
        default:
            if (!running)
                bounce();
            // Clicking the app that's already in front: App Exposé (only
            // when some of its windows are in sight; else they come back)
            if (running && Apps.activeEntry?.id === item.id && Apps.visibleWindowsOf(item.id).length > 0)
                Panels.appExpose(item.id, dock.screen);
            else
                Apps.activateOrLaunch(item);
        }
    }
}
