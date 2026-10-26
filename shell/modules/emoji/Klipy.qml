pragma Singleton
import QtQuick
import Quickshell
import qs.common

// GIFs from KLIPY (docs.klipy.com): trending and search, 24 at a time.
//   GET https://api.klipy.com/api/v1/{app_key}/gifs/trending?page&per_page&customer_id&locale&content_filter&format_filter
//   GET https://api.klipy.com/api/v1/{app_key}/gifs/search?q&page&per_page&customer_id&locale&content_filter&format_filter
// → { result, data: { data: [{ id, slug, title, type, file: { hd|md|sm|xs: { gif|jpg: { url, width, height } } } }], current_page, has_next } }
// The app key is the user's own (partner.klipy.com/api-keys): KLIPY_API_KEY,
// else Config emoji.klipyKey. Nothing is fetched without one.
Singleton {
    id: root

    readonly property string key: Quickshell.env("KLIPY_API_KEY") || (Config.options.emoji?.klipyKey ?? "")
    readonly property bool hasKey: key.trim() !== ""
    readonly property string filter: Config.options.emoji?.gifFilter ?? "medium" // off | low | medium | high

    // Rows { gid, title, preview (still jpg), still (small gif), url (gif to share) }.
    // A ListModel, so loading the next page appends without resetting the grid.
    property ListModel gifs: ListModel {}
    readonly property int count: gifs.count
    function at(i) {
        return i >= 0 && i < gifs.count ? gifs.get(i) : null;
    }
    property string query: "" // what `gifs` are for ("" = trending)
    // idle | loading | ready | error | offline | badKey
    property string status: "idle"
    property int errorCode: 0
    property bool loadingMore: false
    property bool hasNext: false
    property int page: 0

    property var request: null
    property int serial: 0

    // Show trending (q = "") or search results; reuses what's loaded for the same query
    function fetch(q) {
        q = (q ?? "").trim();
        if (!hasKey) {
            status = "idle";
            return;
        }
        if (q === query && (status === "ready" || status === "loading") && gifs.count > 0)
            return;
        query = q;
        gifs.clear();
        page = 0;
        hasNext = false;
        get(1);
    }
    function retry() {
        const q = query;
        query = "\u0000"; // force
        fetch(q);
    }
    function more() {
        if (!hasKey || !hasNext || loadingMore || status !== "ready")
            return;
        get(page + 1);
    }

    function get(p) {
        const s = ++serial; // (first: an aborted request's callback is then ignored)
        if (request) {
            request.abort();
            request = null;
        }
        if (p === 1)
            status = "loading";
        else
            loadingMore = true;
        const country = (Qt.locale().name.split("_")[1] ?? "us").toLowerCase();
        const params = ["page=" + p, "per_page=24", "customer_id=" + encodeURIComponent(EmojiStore.customerId || "irice"), "locale=" + country, "content_filter=" + encodeURIComponent(filter), "format_filter=gif,jpg"];
        if (query !== "")
            params.unshift("q=" + encodeURIComponent(query));
        const url = "https://api.klipy.com/api/v1/" + encodeURIComponent(key.trim()) + "/gifs/" + (query === "" ? "trending" : "search") + "?" + params.join("&");
        const xhr = new XMLHttpRequest();
        request = xhr;
        timeout.restart();
        xhr.onreadystatechange = () => {
            if (xhr.readyState !== XMLHttpRequest.DONE || s !== root.serial)
                return;
            timeout.stop();
            root.request = null;
            root.loadingMore = false;
            if (xhr.status === 0) {
                root.fail("offline", 0, p);
                return;
            }
            if (xhr.status === 401 || xhr.status === 403) {
                root.fail("badKey", xhr.status, p);
                return;
            }
            if (xhr.status !== 200) {
                root.fail("error", xhr.status, p);
                return;
            }
            try {
                const j = JSON.parse(xhr.responseText);
                if (j.result === false) {
                    root.fail("badKey", 0, p);
                    return;
                }
                const d = j.data ?? {};
                const got = (d.data ?? []).filter(g => g.type === undefined || g.type === "gif").map(g => {
                    const f = g.file ?? {};
                    const pick = (sizes, fmt) => {
                        for (const sz of sizes)
                            if (f[sz]?.[fmt]?.url)
                                return f[sz][fmt];
                        return null;
                    };
                    const still = pick(["sm", "xs", "md"], "jpg");
                    const small = pick(["sm", "xs", "md"], "gif");
                    const share = pick(["hd", "md", "sm"], "gif");
                    return { gid: String(g.id ?? g.slug ?? ""), title: g.title ?? "", preview: still?.url ?? small?.url ?? "", still: small?.url ?? "", url: share?.url ?? small?.url ?? "" };
                }).filter(g => g.url !== "");
                if (p === 1)
                    root.gifs.clear();
                root.gifs.append(got);
                root.page = p;
                root.hasNext = !!d.has_next && got.length > 0;
                root.status = "ready";
            } catch (e) {
                root.fail("error", -1, p);
            }
        };
        xhr.open("GET", url);
        xhr.send();
    }

    function fail(kind, code, p) {
        errorCode = code;
        // A failed next page keeps what's shown
        if (p > 1 && kind !== "badKey") {
            hasNext = false;
            return;
        }
        status = kind;
    }

    // Slow or stuck network: give up after 12 s
    Timer {
        id: timeout
        interval: 12000
        onTriggered: {
            if (!root.request)
                return;
            const p = root.loadingMore ? root.page + 1 : 1;
            root.serial++;
            root.request.abort();
            root.request = null;
            root.loadingMore = false;
            root.fail("offline", 0, p);
        }
    }

    // A new key: start over
    onKeyChanged: {
        gifs.clear();
        query = "\u0000";
        status = "idle";
    }
}
