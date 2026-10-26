import QtQuick
import Quickshell
import qs.common
import qs.glass
import qs.services

// Right-click menu for a dock item, above the icon.
ContextMenu {
    id: root

    required property var dock
    readonly property var it: dock.menuItem ?? {}
    readonly property bool pinned: Config.options.dock.pinned.includes(it.id)

    screen: dock.screen
    pointX: dock.menuX
    pointY: dock.screen.height - dock.barHeight - dock.bottomGap - 8
    onDismissed: dock.menuFor = ""

    items: {
        switch (it.kind) {
        case "launcher":
            return [
                { text: "Open App Library", action: () => Panels.show("launchpad", dock.screen) },
                { separator: true },
                { text: "Remove from Dock", action: () => Config.options.dock.appsButton = false }
            ];
        case "minimized":
            return [
                { text: "Restore", action: () => Apps.restore(it.minimized) },
                { separator: true },
                { text: "Close Window", action: () => it.minimized?.wayland?.close() }
            ];
        case "downloads":
            return [
                { text: "Open Downloads", action: () => Qt.openUrlExternally("file://" + Quickshell.env("HOME") + "/Downloads") },
                { text: "Show Recent Files", action: () => dock.stackOpen = true },
                { separator: true },
                { text: "Remove from Dock", action: () => Config.options.dock.downloads = false }
            ];
        case "trash":
            return [
                { text: "Open Trash", action: () => Qt.openUrlExternally("trash:///") },
                { separator: true },
                // Emptying the trash can't be undone, so it asks once
                { text: "Empty Trash…", enabled: (it.trashCount ?? 0) > 0, danger: true, confirm: "Delete " + (it.trashCount ?? 0) + ((it.trashCount ?? 0) === 1 ? " item" : " items") + " for good", action: () => Quickshell.execDetached(["gio", "trash", "--empty"]) }
            ];
        default:
            return [
                { text: it.running ? "New Window" : "Open", action: () => Apps.launch(it.entry) },
                { separator: true },
                { text: root.pinned ? "Remove from Dock" : "Keep in Dock", action: () => root.pinned ? Apps.unpin(it.id) : Apps.pin(it.id) },
                ...(it.running ? [{ separator: true }, { text: "Quit", action: () => Apps.quit(it.id) }] : [])
            ];
        }
    }
}
