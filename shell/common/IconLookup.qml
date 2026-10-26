pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Icon paths that always load. Quickshell.iconPath(name, "fallback") hands
// back "name?fallback=…", which the icon provider can't parse, so a missing
// icon showed as a pink and black box. This checks first, tries the usual
// name variants, then falls back to a generic icon (or "", which draws
// nothing, never a broken texture).
// Names are looked up in iRice's own index of the icon theme first
// (assets/scripts/icon-index.py, cached in ~/.cache/irice): Qt's theme lookup
// depends on the platform theme and desktop settings and can find nothing
// (blank Downloads, Trash and file icons); file paths always load.
Singleton {
    id: root

    readonly property string theme: Quickshell.env("QS_ICON_THEME") || "hicolor"
    readonly property string cacheFile: (Quickshell.env("XDG_CACHE_HOME") || Quickshell.env("HOME") + "/.cache") + "/irice/icon-index-" + theme + ".tsv"
    property var index: ({}) // name -> file
    FileView {
        id: cache
        path: root.cacheFile
        blockLoading: false
        onLoaded: {
            const m = {};
            for (const l of text().split("\n")) {
                const t = l.indexOf("\t");
                if (t > 0)
                    m[l.slice(0, t)] = l.slice(t + 1);
            }
            root.index = m;
        }
    }
    // Refresh the index in the background (quick when nothing changed)
    Process {
        running: true
        command: ["nice", "-n", "19", "python3", "-I", decodeURIComponent(Qt.resolvedUrl("../assets/scripts/icon-index.py").toString().replace("file://", "")), root.theme, root.cacheFile]
        onExited: cache.reload()
    }

    // name and fallback may each be a name or a list of names to try in turn
    function path(name, fallback) {
        const names = [].concat(name ?? []).map(n => String(n)).filter(n => n !== "");
        for (const n of names) {
            if (n.startsWith("/"))
                return "file://" + n; // desktop files sometimes give a file
            if (n.startsWith("file://") || n.startsWith("image://"))
                return n;
        }
        const tried = new Set();
        const find = n => {
            if (!n || tried.has(n))
                return "";
            tried.add(n);
            const f = root.index[n];
            if (f)
                return "file://" + f.split("/").map(encodeURIComponent).join("/");
            return Quickshell.iconPath(n, true);
        };
        // The names, then the usual variants (lowercase, the last part of
        // reverse-DNS ids like org.gnome.Nautilus, dashes for spaces)
        for (const n of names) {
            const parts = n.split(".");
            const last = parts[parts.length - 1];
            for (const v of [n, n.toLowerCase(), last, last.toLowerCase(), n.replace(/\s+/g, "-").toLowerCase()]) {
                const p = find(v);
                if (p !== "")
                    return p;
            }
        }
        const fallbacks = [].concat(fallback ?? []).filter(f => !!f);
        for (const f of fallbacks) {
            const p = find(f);
            if (p !== "")
                return p;
        }
        // Some themes (Adwaita) only ship symbolic versions of places and
        // actions; better than nothing
        for (const n of names.concat(fallbacks)) {
            const p = find(n + "-symbolic");
            if (p !== "")
                return p;
        }
        return find("application-x-executable") || find("application-default-icon") || "";
    }
}
