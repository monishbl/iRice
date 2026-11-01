import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.services

// What tells a real drag of files from a held button: only something being
// dragged can enter a drop area, never a selection, a scrollbar or a window
// edge; and only files open the shelf (not a web image or link, or text). While the button is held (Shelf.probing), each screen gets
// a thin invisible strip at its left and right edges, and a square under
// the pointer just after a shake (Shelf.probe); a drag entering one is what
// opens the shelf. They take no clicks: a held button keeps the pointer
// with whatever it was pressed on, and they go when it comes up. Something
// dropped on one goes onto the shelf.
Scope {
    id: root

    component Probe: PanelWindow {
        id: probe
        property string where: "" // left | right | shake
        color: "transparent"
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "irice:shelf-probe"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        DropArea {
            anchors.fill: parent
            // (files only: a browser's image or link, or selected text, is
            // dragged around all the time without meaning the shelf)
            onEntered: d => {
                let files = false;
                for (const u of d.hasUrls ? d.urls : [])
                    if (String(u).startsWith("file:"))
                        files = true;
                if (!files) {
                    d.accepted = false;
                    return;
                }
                d.accept(Qt.CopyAction);
                Shelf.dragArrived(probe.where, probe.screen, d.y);
            }
            onExited: Shelf.dragLeft(probe.where)
            onDropped: d => {
                Shelf.show(probe.where === "shake" ? "shake" : "edge", probe.screen);
                if (Shelf.drop(d.urls, d.text))
                    d.accept(Qt.CopyAction);
            }
        }
    }

    Variants {
        model: Quickshell.screens
        Scope {
            id: perScreen
            required property var modelData
            readonly property bool edges: Shelf.probing && Shelf.cfg.edge
            LazyLoader {
                active: perScreen.edges
                Probe {
                    where: "left"
                    screen: perScreen.modelData
                    anchors { left: true; top: true; bottom: true }
                    implicitWidth: 3
                }
            }
            LazyLoader {
                active: perScreen.edges
                Probe {
                    where: "right"
                    screen: perScreen.modelData
                    anchors { right: true; top: true; bottom: true }
                    implicitWidth: 3
                }
            }
            // (just after a shake: under the pointer, where the drag is)
            LazyLoader {
                active: Shelf.probing && Shelf.probe !== null && Shelf.probe.screen === perScreen.modelData
                Probe {
                    where: "shake"
                    screen: perScreen.modelData
                    anchors { left: true; top: true }
                    margins.left: Math.max(0, (Shelf.probe?.x ?? 0) - perScreen.modelData.x - 180)
                    margins.top: Math.max(0, (Shelf.probe?.y ?? 0) - perScreen.modelData.y - 180)
                    implicitWidth: 360
                    implicitHeight: 360
                }
            }
        }
    }
}
