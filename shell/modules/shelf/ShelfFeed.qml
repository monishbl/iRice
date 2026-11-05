import QtQuick
import qs.common
import qs.services

// While tessera-ui draws the shelf (rust.surfaces: shelf), what the shelf
// is (services/Shelf.qml) reaches it here, one line on its stdin
// (RustUi.tell): opened, its items, the screen and side, the drop zone. And
// what it asks of this shell (RustUi's "shelf" call) is done here: a drop on
// the card or the zone's targets, removing, clearing, closing, Share, and
// whether the pointer or a drag is over the card or the zone.
// (native/tessera-ui/src/surfaces/shelf reads what this sends)
Item {
    id: root

    readonly property var feed: ({
        opened: Shelf.opened,
        items: Shelf.items.map(i => ({ uri: String(i.uri ?? ""), path: String(i.path ?? ""), name: String(i.name ?? ""), kind: String(i.kind ?? "file"), text: String(i.text ?? "") })),
        screen: Shelf.screen?.name ?? "",
        side: Shelf.side,
        atY: Shelf.atY,
        zone: Shelf.zone,
        zoneScreen: Shelf.zoneScreen?.name ?? ""
    })
    function send() {
        RustUi.tell("shelf", { state: root.feed });
    }
    onFeedChanged: send()
    Component.onCompleted: send()

    function strings(a) {
        return Array.isArray(a) ? a.filter(u => typeof u === "string") : [];
    }

    Connections {
        target: RustUi
        // (sent again when tessera-ui starts, or starts drawing the shelf)
        function onStarted() {
            root.send();
        }
        function onDrawnChanged() {
            root.send();
        }
        function onFeedCall(surface, what, a) {
            if (surface !== "shelf")
                return;
            switch (what) {
            case "drop":
                Shelf.drop(root.strings(a[0]), String(a[1] ?? ""));
                break;
            case "removeAt": {
                const i = Number(a[0]);
                if (Number.isInteger(i) && i >= 0 && i < Shelf.count)
                    Shelf.removeAt(i);
                break;
            }
            case "clear":
                Shelf.clear();
                break;
            case "hide":
                Shelf.hide();
                break;
            case "share":
                if (Shelf.paths.length > 0)
                    Panels.share(Shelf.paths, Shelf.screen ?? undefined);
                break;
            case "hovered":
                Shelf.hovered = a[0] === true;
                break;
            case "zoneHover":
                Shelf.zoneHover(a[0] === true);
                break;
            case "cancelZone":
                Shelf.cancelZone();
                break;
            case "keep":
                Shelf.keep(root.strings(a[0]), String(a[1] ?? ""));
                break;
            case "sendOn":
                Shelf.sendOn(root.strings(a[0]));
                break;
            }
        }
    }
}
