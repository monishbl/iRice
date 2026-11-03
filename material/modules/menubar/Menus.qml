import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.common
import qs.glass
import qs.services

// The menu bar's dropdowns: system menu, the active app's menu, Window menu,
// tray menus. Glass draws them with MenuPopover, Material with MaterialMenu
// (each look's loader stays empty in the other); the items are shared.
Scope {
    id: root

    function systemItems(screen) {
        return [
            { text: "About This Computer", action: () => Panels.show("about", screen) },
            { separator: true },
            { text: "System Settings…", shortcut: Updates.count > 0 ? Updates.count + (Updates.count === 1 ? " update" : " updates") : "", action: () => Panels.openSettings() },
            ...(Updates.count > 0 ? [{ text: "Install " + Updates.count + (Updates.count === 1 ? " Update…" : " Updates…"), action: () => Updates.install() }] : []),
            { separator: true },
            // Duplicated bars or a stuck shell: restarts only the shell, apps keep running
            { text: "Repair Shell", shortcut: KeyNames.combo("SUPER CTRL Escape"), action: () => System.repairShell() },
            { separator: true },
            { text: "Sleep", action: () => System.power("sleep") },
            { text: "Restart…", action: () => Panels.show("session", screen) },
            { text: "Shut Down…", action: () => Panels.show("session", screen) },
            { separator: true },
            { text: "Lock Screen", action: () => System.power("lock") },
            { text: "Log Out " + (Quickshell.env("USER") ?? "") + "…", action: () => Panels.show("session", screen) }
        ];
    }

    function appItems() {
        const entry = Apps.activeEntry, name = Apps.activeName;
        return [
            { text: "New " + name + " Window", enabled: !!entry, action: () => Apps.launch(entry) },
            { separator: true },
            { text: "Quit " + name, enabled: !!Apps.active, action: () => Apps.quit(Apps.active?.appId) }
        ];
    }

    function windowItems() {
        const win = Apps.activeHyprland;
        const has = !!win;
        const floating = win?.lastIpcObject?.floating ?? false;
        const pinned = win?.lastIpcObject?.pinned ?? false;
        const grouped = (win?.lastIpcObject?.grouped?.length ?? 0) > 1;
        const tiles = names => names.map(n => ({ frac: Tiling.layouts[n], label: Tiling.names[n], action: () => Tiling.snap(n) }));
        return [
            { text: "Minimize", enabled: has, action: () => Apps.minimize() },
            { text: "Float", checked: floating, enabled: has, action: () => Apps.windowAction("float", { action: "toggle" }) },
            { text: "Keep on All Spaces", checked: pinned, enabled: has && floating, action: () => Apps.windowAction("pin", { action: "toggle" }) },
            { text: grouped ? "Move Tab to New Window" : "Merge into Tabs", shortcut: KeyNames.combo("SUPER T"), enabled: has, action: () => grouped ? Hyprland.dispatch("hl.dsp.window.move({ out_of_group = true })") : Apps.mergeIntoTabs() },
            { text: "Picture in Picture", checked: has && Tiling.isPip(win), shortcut: KeyNames.combo("SUPER SHIFT P"), enabled: has, action: () => Tiling.pip() },
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
        ];
    }

    // A tray menu's entries; submenus drill down in place (`menu.handle`)
    function trayItems(opener, menu) {
        return opener.children.values.map(e => e.isSeparator ? { separator: true } : {
            text: e.text + (e.hasChildren ? "  ›" : ""),
            enabled: e.enabled,
            checked: e.checkState === Qt.Checked,
            keepOpen: e.hasChildren,
            entry: e, // (rows are kept only for the same entries: see MenuPopover)
            action: () => {
                if (e.hasChildren)
                    menu.handle = e;
                else
                    e.triggered();
            }
        });
    }
    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: mSlot0
            required property var modelData
            panel: "systemMenu"
            screen: modelData
            active: alive && Theme.material

            MaterialMenu {
                screen: mSlot0.modelData
                popoverName: "systemMenu"
                items: root.systemItems(mSlot0.modelData)
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: mSlot1
            required property var modelData
            panel: "appMenu"
            screen: modelData
            active: alive && Theme.material

            MaterialMenu {
                screen: mSlot1.modelData
                popoverName: "appMenu"
                items: root.appItems()
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: mSlot2
            required property var modelData
            panel: "windowMenu"
            screen: modelData
            active: alive && Theme.material

            MaterialMenu {
                screen: mSlot2.modelData
                popoverName: "windowMenu"
                items: root.windowItems()
            }
        }
    }

    Variants {
        model: Quickshell.screens

        PanelLoader {
            id: mSlot3
            required property var modelData
            panel: "trayMenu"
            screen: modelData
            active: alive && Theme.material

            MaterialMenu {
                id: mTray
                property var handle: Panels.trayMenu
                screen: mSlot3.modelData
                popoverName: "trayMenu"
                onShownChanged: if (shown) handle = Panels.trayMenu

                QsMenuOpener {
                    id: mOpener
                    menu: mTray.handle
                }

                items: root.trayItems(mOpener, mTray)
            }
        }
    }
}
