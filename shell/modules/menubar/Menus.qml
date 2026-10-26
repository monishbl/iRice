import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.glass
import qs.services

// The menu bar's dropdowns: system menu, the active app's menu, Window menu.
Scope {
    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: slot0
            required property var modelData
            panel: "systemMenu"
            screen: modelData

            MenuPopover {
                readonly property var modelData: slot0.modelData
                screen: modelData
                popoverName: "systemMenu"
                items: [
                    { text: "About This Computer", action: () => Panels.show("about", modelData) },
                    { separator: true },
                    { text: "System Settings…", shortcut: Updates.count > 0 ? Updates.count + (Updates.count === 1 ? " update" : " updates") : "", action: () => Panels.openSettings() },
                    ...(Updates.count > 0 ? [{ text: "Install " + Updates.count + (Updates.count === 1 ? " Update…" : " Updates…"), action: () => Updates.install() }] : []),
                    { separator: true },
                    // Duplicated bars or a stuck shell: restarts only the shell, apps keep running
                    { text: "Repair Shell", shortcut: KeyNames.combo("SUPER CTRL Escape"), action: () => System.repairShell() },
                    { separator: true },
                    { text: "Sleep", action: () => System.power("sleep") },
                    { text: "Restart…", action: () => Panels.show("session", modelData) },
                    { text: "Shut Down…", action: () => Panels.show("session", modelData) },
                    { separator: true },
                    { text: "Lock Screen", action: () => System.power("lock") },
                    { text: "Log Out " + (Quickshell.env("USER") ?? "") + "…", action: () => Panels.show("session", modelData) }
                ]
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: slot1
            required property var modelData
            panel: "appMenu"
            screen: modelData

            MenuPopover {
                readonly property var modelData: slot1.modelData
                readonly property var entry: Apps.activeEntry
                readonly property string name: Apps.activeName
                screen: modelData
                popoverName: "appMenu"
                items: [
                    { text: "New " + name + " Window", enabled: !!entry, action: () => Apps.launch(entry) },
                    { separator: true },
                    { text: "Quit " + name, enabled: !!Apps.active, action: () => Apps.quit(Apps.active?.appId) }
                ]
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: slot2
            required property var modelData
            panel: "windowMenu"
            screen: modelData

            MenuPopover {
                readonly property var modelData: slot2.modelData
                readonly property var win: Apps.activeHyprland
                readonly property bool has: !!win
                readonly property bool floating: win?.lastIpcObject?.floating ?? false
                readonly property bool pinned: win?.lastIpcObject?.pinned ?? false
                function tiles(names) {
                    return names.map(n => ({ frac: Tiling.layouts[n], label: Tiling.names[n], action: () => Tiling.snap(n) }));
                }
                screen: modelData
                popoverName: "windowMenu"
                items: [
                    { text: "Minimize", enabled: has, action: () => Apps.minimize() },
                    { text: "Float", checked: floating, enabled: has, action: () => Apps.windowAction("float", { action: "toggle" }) },
                    { text: "Keep on All Spaces", checked: pinned, enabled: has && floating, action: () => Apps.windowAction("pin", { action: "toggle" }) },
                    { separator: true },
                    { header: "Move & Resize" },
                    { tiles: tiles(["left", "right", "top", "bottom"]) },
                    { tiles: tiles(["topLeft", "topRight", "bottomLeft", "bottomRight"]) },
                    { tiles: tiles(["leftThird", "centerThird", "rightThird", "leftTwoThirds"]) },
                    { text: "Fill", shortcut: KeyNames.combo("SUPER CTRL F"), enabled: has, action: () => Tiling.snap("fill") },
                    { text: "Center", shortcut: KeyNames.combo("SUPER CTRL C"), enabled: has, action: () => Tiling.snap("center") },
                    { text: "Return to Previous Size", shortcut: KeyNames.combo("SUPER CTRL R"), enabled: has && Tiling.canRestore(win), action: () => Tiling.restore() },
                    { header: "Arrange" },
                    { text: "Left & Right", action: () => Tiling.arrange("leftRight") },
                    { text: "Top & Bottom", action: () => Tiling.arrange("topBottom") },
                    { text: "Quarters", action: () => Tiling.arrange("quarters") },
                    { separator: true },
                    { text: "Zoom", enabled: has, action: () => Apps.windowAction("fullscreen", { mode: "maximized", action: "toggle" }) },
                    { text: "Enter Full Screen", enabled: has, action: () => Apps.windowAction("fullscreen", { mode: "fullscreen", action: "toggle" }) },
                    { separator: true },
                    { text: "Move to Space 1", enabled: has, action: () => Apps.windowAction("move", { workspace: 1, follow: false }) },
                    { text: "Move to Space 2", enabled: has, action: () => Apps.windowAction("move", { workspace: 2, follow: false }) },
                    { text: "Move to Space 3", enabled: has, action: () => Apps.windowAction("move", { workspace: 3, follow: false }) },
                    { separator: true },
                    { text: "Close Window", enabled: has, action: () => Apps.windowAction("close") }
                ]
            }
        }
    }

    // Tray menus. Submenus drill down in place.
    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: slot3
            required property var modelData
            panel: "trayMenu"
            screen: modelData

            MenuPopover {
                id: tray
                readonly property var modelData: slot3.modelData
                property var handle: Panels.trayMenu
                screen: modelData
                popoverName: "trayMenu"
                onShownChanged: if (shown) handle = Panels.trayMenu

                QsMenuOpener {
                    id: opener
                    menu: tray.handle
                }

                items: opener.children.values.map(e => e.isSeparator ? { separator: true } : {
                    text: e.text + (e.hasChildren ? "  ›" : ""),
                    enabled: e.enabled,
                    checked: e.checkState === Qt.Checked,
                    keepOpen: e.hasChildren,
                    action: () => {
                        if (e.hasChildren)
                            tray.handle = e;
                        else
                            e.triggered();
                    }
                })
            }
        }
    }
}
