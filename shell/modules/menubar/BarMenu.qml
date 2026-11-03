import QtQuick
import Quickshell
import qs.common
import qs.glass

// One menu bar menu on one screen, drawn by the look's own menu: Glass's
// MenuPopover, Material's MaterialMenu (the other look's loader stays
// empty). Both take the same `items`. A tray menu (`tray`) lists the open
// tray item's entries instead, and its submenus drill down in place.
//   BarMenu { panel: "appMenu"; screen: modelData; items: [...] }
Scope {
    id: root

    required property string panel
    required property var screen
    property var items: []
    property bool tray: false

    // A tray menu's entries (`opener`, on the menu `menu`): a submenu
    // replaces them with its own (`menu.handle`)
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

    PanelLoader {
        panel: root.panel
        screen: root.screen
        active: alive && !Theme.material

        MenuPopover {
            id: glassMenu
            property var handle: Panels.trayMenu
            screen: root.screen
            popoverName: root.panel
            onShownChanged: if (shown) handle = Panels.trayMenu

            QsMenuOpener {
                id: glassOpener
                menu: root.tray ? glassMenu.handle : null
            }

            items: root.tray ? root.trayItems(glassOpener, glassMenu) : root.items
        }
    }

    PanelLoader {
        panel: root.panel
        screen: root.screen
        active: alive && Theme.material

        MaterialMenu {
            id: materialMenu
            property var handle: Panels.trayMenu
            screen: root.screen
            popoverName: root.panel
            onShownChanged: if (shown) handle = Panels.trayMenu

            QsMenuOpener {
                id: materialOpener
                menu: root.tray ? materialMenu.handle : null
            }

            items: root.tray ? root.trayItems(materialOpener, materialMenu) : root.items
        }
    }
}
