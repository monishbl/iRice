import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Io
import qs.common
import qs.glass
import qs.services

// Files and folders on the desktop (~/Desktop), on the widgets' grid: each
// widget cell holds 2 × 2 icon spots, and widgets keep their cells. New
// icons line up from the top right, like macOS. Click to select, double-click
// to open, drag to move (it snaps to a free spot), right-click for Open,
// Quick Look, Move to Trash and more. With Use Stacks, files are grouped by
// kind into stacks that fan open on click.
Item {
    id: root

    required property var desk // DesktopWidgets: grid, widgets, menu
    anchors.fill: parent
    readonly property var cfg: Config.options.desktop
    // Your Desktop folder (user-dirs), made if it doesn't exist yet; "" until
    // known (a folder model with a missing folder lists the working directory)
    property string dir: ""
    Process {
        running: true
        command: ["sh", "-c", "d=$(xdg-user-dir DESKTOP 2>/dev/null); [ -n \"$d\" ] && [ \"$d\" != \"$HOME\" ] || d=\"$HOME/Desktop\"; mkdir -p \"$d\" && printf '%s' \"$d\""]
        stdout: StdioCollector {
            onStreamFinished: root.dir = text.trim().replace(/\/$/, "")
        }
    }
    readonly property real spot: (desk.defs.cell + desk.defs.gap) / 2 // an icon spot
    readonly property int cols: desk.cols * 2
    readonly property int rows: desk.rows * 2
    property string selected: ""
    property string openStack: ""

    FolderListModel {
        id: files
        folder: root.cfg.icons && root.dir !== "" ? "file://" + root.dir : ""
        showDirsFirst: true
        showHidden: false
        sortField: FolderListModel.Name
    }
    readonly property var entries: {
        files.count; // re-read when the folder changes
        if (root.dir === "" || !root.cfg.icons)
            return [];
        const out = [];
        for (let i = 0; i < files.count; i++)
            out.push({ name: files.get(i, "fileName"), path: files.get(i, "filePath"), dir: files.get(i, "fileIsDir"), modified: files.get(i, "fileModified") });
        return out;
    }

    // ── Stacks ──────────────────────────────────────────────────────────
    function kindOf(e) {
        if (e.dir)
            return "Folders";
        if (/^Screenshot/i.test(e.name))
            return "Screenshots";
        const x = e.name.split(".").pop().toLowerCase();
        if (["png", "jpg", "jpeg", "webp", "gif", "svg", "heic", "avif", "bmp"].includes(x)) return "Images";
        if (["mp4", "mkv", "webm", "mov", "avi"].includes(x)) return "Movies";
        if (["mp3", "flac", "ogg", "wav", "m4a", "opus"].includes(x)) return "Music";
        if (["pdf", "doc", "docx", "odt", "txt", "md", "rtf", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp", "epub"].includes(x)) return "Documents";
        if (["zip", "tar", "gz", "xz", "7z", "rar", "zst", "bz2"].includes(x)) return "Archives";
        return "Other";
    }
    // What's shown: files, or one stack per kind (most recent file on top)
    readonly property var items: {
        if (!cfg.useStacks)
            return entries.map(e => ({ key: e.path, name: e.name, path: e.path, dir: e.dir }));
        const groups = {};
        for (const e of entries)
            (groups[kindOf(e)] = groups[kindOf(e)] ?? []).push(e);
        const order = ["Folders", "Documents", "Images", "Screenshots", "Movies", "Music", "Archives", "Other"];
        return order.filter(k => groups[k]).map(k => ({ key: "stack:" + k, name: k, stack: groups[k].sort((a, b) => b.modified - a.modified) }));
    }

    // ── Places on the grid ──────────────────────────────────────────────
    // Saved spots: "key\tcol\trow" (in icon spots)
    readonly property var saved: {
        const m = {};
        for (const l of cfg.iconPositions) {
            const [k, c, r] = l.split("\t");
            m[k] = { col: Number(c), row: Number(r) };
        }
        return m;
    }
    // Spots covered by widgets
    readonly property var blocked: {
        const b = new Set();
        for (const w of desk.placed) {
            const u = desk.defs.units[w.size];
            for (let c = w.col * 2; c < (w.col + u[0]) * 2; c++)
                for (let r = w.row * 2; r < (w.row + u[1]) * 2; r++)
                    b.add(c + "," + r);
        }
        return b;
    }
    readonly property var placed: {
        const taken = new Set(blocked), out = {};
        const free = s => s && s.col >= 0 && s.row >= 0 && s.col < cols && s.row < rows && !taken.has(s.col + "," + s.row);
        for (const it of items) {
            const s = saved[it.key];
            if (free(s)) {
                out[it.key] = s;
                taken.add(s.col + "," + s.row);
            }
        }
        // The rest: top right, down each column, then the next column left
        for (const it of items) {
            if (out[it.key])
                continue;
            search: for (let c = cols - 1; c >= 0; c--)
                for (let r = 0; r < rows; r++)
                    if (!taken.has(c + "," + r)) {
                        out[it.key] = { col: c, row: r };
                        taken.add(c + "," + r);
                        break search;
                    }
        }
        return out;
    }
    function moveTo(key, x, y) {
        let col = Math.round((x - desk.origin.x) / spot), row = Math.round((y - desk.origin.y) / spot);
        const taken = new Set(blocked);
        for (const [k, s] of Object.entries(placed))
            if (k !== key)
                taken.add(s.col + "," + s.row);
        // Nearest free spot to the drop
        let best = null, bestD = 1e9;
        for (let c = 0; c < cols; c++)
            for (let r = 0; r < rows; r++)
                if (!taken.has(c + "," + r)) {
                    const d = (c - col) ** 2 + (r - row) ** 2;
                    if (d < bestD) {
                        bestD = d;
                        best = { col: c, row: r };
                    }
                }
        if (!best)
            return;
        const keep = cfg.iconPositions.filter(l => !l.startsWith(key + "\t") && items.some(i => l.startsWith(i.key + "\t")));
        Config.options.desktop.iconPositions = keep.concat([key + "\t" + best.col + "\t" + best.row]);
    }

    // ── Actions ─────────────────────────────────────────────────────────
    function open(path) {
        Files.open(path);
    }
    function trash(path) {
        const name = path.split("/").pop();
        Quickshell.execDetached(["sh", "-c", "gio trash -- \"$1\" || exit; a=$(notify-send -a Finder -i user-trash -t 8000 -A undo=Undo 'Moved to Trash' \"$2\"); [ \"$a\" = undo ] && gio trash --restore \"trash:///$(basename \"$1\")\"", "sh", path, name]);
        Sounds.play("trash");
    }
    function menuFor(it, x, y) {
        selected = it.key;
        const p = it.path;
        const items = it.stack ? [
            { text: "Open " + it.name, action: () => root.openStack = it.key },
            { text: "Share " + it.stack.length + (it.stack.length === 1 ? " file…" : " files…"), action: () => Panels.share(it.stack.map(f => f.path), desk.screen) },
            { separator: true },
            { text: "✓ Use Stacks", action: () => Config.options.desktop.useStacks = false }
        ] : [
            { text: "Open", action: () => root.open(p) },
            { text: "Quick Look", action: () => Panels.quickLook(p) },
            { text: "Share…", action: () => Panels.share([p], desk.screen) },
            { text: "Show in Files", action: () => Files.reveal(p) },
            { text: "Copy Path", action: () => Quickshell.execDetached(["wl-copy", p]) },
            { separator: true },
            { text: "Move to Trash", danger: true, action: () => root.trash(p) }
        ];
        desk.menu = { items: items, x: x, y: y };
    }

    // Clicking the bare desktop clears the selection and closes a stack
    TapHandler {
        onTapped: {
            root.selected = "";
            root.openStack = "";
        }
    }

    Repeater {
        model: root.items
        Item {
            id: icon
            required property var modelData
            readonly property var place: root.placed[modelData.key] ?? { col: 0, row: 0 }
            readonly property bool isSelected: root.selected === modelData.key
            readonly property real homeX: root.desk.origin.x + place.col * root.spot
            readonly property real homeY: root.desk.origin.y + place.row * root.spot
            x: homeX
            y: homeY
            width: root.spot - 6
            height: root.spot - 2
            z: drag.active ? 5 : 0
            // Lock and unlock: zooms with the widgets (LockFx)
            readonly property real fxScale: LockFx.active ? LockFx.itemScale(x + width / 2, y + height / 2, root.width, root.height) : 1
            opacity: LockFx.active ? LockFx.itemOpacity(x + width / 2, y + height / 2, root.width, root.height) : 1
            transform: Scale {
                origin.x: icon.parent.width / 2 - icon.x
                origin.y: icon.parent.height / 2 - icon.y
                xScale: icon.fxScale
                yScale: icon.fxScale
            }
            Behavior on x { enabled: !drag.active; SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.3 } }
            Behavior on y { enabled: !drag.active; SpringAnimation { spring: 5; damping: 0.4; epsilon: 0.3 } }

            // Selection: a soft tile behind the picture, the name highlighted
            Squircle {
                anchors.horizontalCenter: parent.horizontalCenter
                y: 0
                width: 54
                height: 54
                radius: 10
                color: Qt.rgba(0, 0, 0, 0.22)
                visible: icon.isSelected
            }
            Item {
                id: picture
                anchors.horizontalCenter: parent.horizontalCenter
                y: 3
                width: 48
                height: 48
                // A stack: its three newest files, fanned
                Repeater {
                    model: icon.modelData.stack ? icon.modelData.stack.slice(0, 3).reverse() : []
                    AppIcon {
                        required property var modelData
                        required property int index
                        anchors.centerIn: parent
                        anchors.verticalCenterOffset: -index * 3
                        rotation: (index - 1) * 6
                        size: 46
                        styled: false
                        source: IconLookup.path(modelData.dir ? "folder" : Search.iconForFile(modelData.path), "text-x-generic")
                    }
                }
                // An image: its thumbnail; anything else: its icon
                readonly property bool image: !icon.modelData.stack && !icon.modelData.dir && root.kindOf(icon.modelData) === "Images"
                Image {
                    id: thumb
                    anchors.fill: parent
                    visible: picture.image && status === Image.Ready
                    source: picture.image ? "file://" + icon.modelData.path : ""
                    sourceSize: Qt.size(104, 104)
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                }
                AppIcon {
                    anchors.fill: parent
                    visible: !icon.modelData.stack && !thumb.visible
                    size: 48
                    styled: false
                    source: icon.modelData.stack ? "" : IconLookup.path(icon.modelData.dir ? "folder" : Search.iconForFile(icon.modelData.path), "text-x-generic")
                }
            }
            Rectangle {
                anchors { horizontalCenter: parent.horizontalCenter; top: picture.bottom; topMargin: 3 }
                width: Math.min(parent.width, label.implicitWidth + 8)
                height: label.implicitHeight + 2
                radius: 4
                color: icon.isSelected ? Theme.accent : "transparent"
                Text {
                    id: label
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, icon.width - 8)
                    horizontalAlignment: Text.AlignHCenter
                    text: icon.modelData.name
                    elide: Text.ElideMiddle
                    maximumLineCount: 2
                    wrapMode: Text.WrapAnywhere
                    color: "white"
                    style: icon.isSelected ? Text.Normal : Text.Raised
                    styleColor: Qt.rgba(0, 0, 0, 0.55)
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.Medium }
                }
            }

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: m => {
                    if (m.button === Qt.RightButton) {
                        const p = mapToItem(root, m.x, m.y);
                        root.menuFor(icon.modelData, p.x, p.y);
                    } else if (icon.modelData.stack) {
                        root.openStack = root.openStack === icon.modelData.key ? "" : icon.modelData.key;
                    } else {
                        root.selected = icon.modelData.key;
                    }
                }
                onDoubleClicked: if (!icon.modelData.stack) root.open(icon.modelData.path)
            }
            DragHandler {
                id: drag
                target: icon
                dragThreshold: 6
                onActiveChanged: {
                    if (active) {
                        root.selected = icon.modelData.key;
                        return;
                    }
                    root.moveTo(icon.modelData.key, icon.x, icon.y);
                    icon.x = Qt.binding(() => icon.homeX);
                    icon.y = Qt.binding(() => icon.homeY);
                }
            }
        }
    }

    // An open stack: its files in a glass panel beside it
    Item {
        id: stackPanel
        readonly property var it: root.items.find(i => i.key === root.openStack) ?? null
        readonly property var where: it ? root.placed[it.key] : null
        visible: it !== null
        readonly property int perRow: 4
        readonly property int rowsShown: Math.min(4, Math.ceil((it?.stack?.length ?? 0) / perRow))
        width: perRow * 96 + 24 + 2 * glass.shadowMargin
        height: rowsShown * 96 + 48 + 2 * glass.shadowMargin
        x: where ? Math.max(8, root.desk.origin.x + where.col * root.spot - width + glass.shadowMargin + 8) : 0
        y: where ? Math.min(root.height - height - 100, root.desk.origin.y + where.row * root.spot - glass.shadowMargin) : 0
        Glass {
            id: glass
            anchors.fill: parent
            radius: 18
            shadowMargin: 20
            forceLiquid: true
        }
        MouseArea { anchors.fill: parent; anchors.margins: glass.shadowMargin } // clicks stay inside
        Text {
            x: glass.shadowMargin + 14
            y: glass.shadowMargin + 10
            text: stackPanel.it?.name ?? ""
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
        }
        Grid {
            x: glass.shadowMargin + 12
            y: glass.shadowMargin + 36
            columns: stackPanel.perRow
            Repeater {
                model: (stackPanel.it?.stack ?? []).slice(0, 16)
                Item {
                    id: cell
                    required property var modelData
                    width: 96
                    height: 96
                    Squircle {
                        anchors.fill: parent
                        anchors.margins: 2
                        radius: 10
                        color: cellMouse.containsMouse ? Theme.color.fill : "transparent"
                    }
                    AppIcon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 10
                        size: 48
                        styled: false
                        source: IconLookup.path(cell.modelData.dir ? "folder" : Search.iconForFile(cell.modelData.path), "text-x-generic")
                    }
                    Text {
                        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 8 }
                        width: parent.width - 8
                        horizontalAlignment: Text.AlignHCenter
                        text: cell.modelData.name
                        elide: Text.ElideMiddle
                        color: Theme.color.label
                        font { family: Theme.font.family; pixelSize: Theme.font.small }
                    }
                    MouseArea {
                        id: cellMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onClicked: m => {
                            if (m.button === Qt.RightButton) {
                                const p = mapToItem(root, m.x, m.y);
                                root.menuFor({ key: cell.modelData.path, name: cell.modelData.name, path: cell.modelData.path, dir: cell.modelData.dir }, p.x, p.y);
                            } else {
                                root.openStack = "";
                                root.open(cell.modelData.path);
                            }
                        }
                    }
                }
            }
        }
    }
}
