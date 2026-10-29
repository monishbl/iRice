import QtQuick
import Quickshell.Widgets
import qs.common
import qs.services

// Anime image browser for yande.re, Konachan and Danbooru. Safe-rated results
// only unless "Explicit" is switched on. Click an image to open it; scroll to
// the end to load more.
Item {
    id: root

    readonly property var providers: [
        { id: "yandere", name: "yande.re" },
        { id: "konachan", name: "Konachan" },
        { id: "danbooru", name: "Danbooru" }
    ]
    readonly property string provider: Config.options.sidebar.booruProvider
    property var posts: []
    property int page: 1
    property bool loading: false
    property string error: ""
    property bool exhausted: false

    function url(tags, page) {
        const provider = Config.options.sidebar.booruProvider;
        const nsfw = Config.options.sidebar.booruNsfw;
        const t = tags.trim().split(/\s+/).filter(x => x).join(" ");
        if (provider === "danbooru")
            return "https://danbooru.donmai.us/posts.json?limit=20&page=" + page + "&tags=" + encodeURIComponent((t + (nsfw ? "" : " rating:g")).trim());
        const host = provider === "konachan" ? (nsfw ? "https://konachan.com" : "https://konachan.net") : "https://yande.re";
        return host + "/post.json?limit=20&page=" + page + "&tags=" + encodeURIComponent((t + (nsfw ? "" : " rating:s")).trim());
    }

    function normalize(p) {
        const provider = Config.options.sidebar.booruProvider;
        if (provider === "danbooru")
            return { preview: p.preview_file_url, full: p.large_file_url ?? p.file_url, page: "https://danbooru.donmai.us/posts/" + p.id, w: p.image_width, h: p.image_height };
        const host = provider === "konachan" ? "https://konachan.net" : "https://yande.re";
        return { preview: p.preview_url, full: p.sample_url ?? p.file_url, page: host + "/post/show/" + p.id, w: p.width, h: p.height };
    }

    property int generation: 0
    property var request: null
    function search(reset) {
        if (!reset && (loading || exhausted))
            return;
        if (reset) {
            generation++;
            if (request) request.abort();
            page = 1;
            posts = [];
            exhausted = false;
        }
        loading = true;
        error = "";
        const version = generation;
        const x = new XMLHttpRequest();
        request = x;
        x.onreadystatechange = () => {
            if (version !== generation) return;
            if (x.readyState !== XMLHttpRequest.DONE)
                return;
            loading = false;
            if (x.status !== 200) {
                error = x.status === 0 ? "Couldn't reach " + providers.find(p => p.id === provider).name + ". Check your internet connection." : "Search failed (" + x.status + "). Try fewer tags.";
                return;
            }
            try {
                const items = JSON.parse(x.responseText).map(normalize).filter(p => p.preview);
                exhausted = items.length === 0;
                posts = posts.concat(items);
                page++;
            } catch (e) {
                error = "Got an unexpected response. Try again.";
            }
        };
        x.open("GET", url(tags.text, page));
        x.send();
    }

    Component.onCompleted: search(true)

    Column {
        id: controls
        width: parent.width
        spacing: 8

        Segmented {
            width: parent.width
            options: root.providers.map(p => p.name)
            current: root.providers.findIndex(p => p.id === root.provider)
            onSelected: i => {
                Config.options.sidebar.booruProvider = root.providers[i].id;
                root.search(true);
            }
        }

        Row {
            width: parent.width
            spacing: 8
            Rectangle {
                width: parent.width - nsfwRow.width - 8
                height: 32
                radius: 16
                color: Theme.color.fill
                Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "magnifying-glass"; size: 14; color: Theme.color.secondary }
                TextInput {
                    id: tags
                    anchors { fill: parent; leftMargin: 32; rightMargin: 12 }
                    verticalAlignment: TextInput.AlignVCenter
                    color: Theme.color.label
                    clip: true
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                    onAccepted: root.search(true)
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: tags.text === ""
                        text: "Tags, e.g. scenery sky"
                        color: Theme.color.tertiary
                        font: tags.font
                    }
                }
            }
            Row {
                id: nsfwRow
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Explicit"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Switch {
                    anchors.verticalCenter: parent.verticalCenter
                    checked: Config.options.sidebar.booruNsfw
                    onToggled: {
                        Config.options.sidebar.booruNsfw = !Config.options.sidebar.booruNsfw;
                        root.search(true);
                    }
                }
            }
        }
    }

    GridView {
        id: grid
        anchors { top: controls.bottom; topMargin: 10; left: parent.left; right: parent.right; bottom: parent.bottom }
        clip: true
        cellWidth: width / 2
        cellHeight: cellWidth
        model: root.posts
        boundsBehavior: Flickable.StopAtBounds
        onAtYEndChanged: if (atYEnd && count > 0) root.search(false)

        delegate: Item {
            id: cell
            required property var modelData
            width: grid.cellWidth
            height: grid.cellHeight

            ClippingRectangle {
                anchors { fill: parent; margins: 4 }
                radius: 12
                color: Theme.color.fill
                Image {
                    anchors.fill: parent
                    source: cell.modelData.preview
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    sourceSize: Qt.size(width, height)
                    opacity: status === Image.Ready ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: Theme.motion.popover } }
                    scale: imgMouse.containsMouse ? 1.04 : 1
                    Behavior on scale { NumberAnimation { duration: Theme.motion.hover } }
                }
                MouseArea {
                    id: imgMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Launch.open(cell.modelData.page)
                }
            }
        }

        footer: Item {
            width: grid.width
            height: 40
            Text {
                anchors.centerIn: parent
                text: root.loading ? "Loading…" : root.error ? root.error : root.exhausted && root.posts.length === 0 ? "No results. Try different tags." : ""
                width: parent.width - 20
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: root.error ? Theme.color.danger : Theme.color.secondary
                font { family: Theme.font.family; pixelSize: Theme.font.small }
            }
        }
    }
}
