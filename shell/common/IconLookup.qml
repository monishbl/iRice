pragma Singleton
import QtQuick
import Quickshell

// Icon paths that always load. Quickshell.iconPath(name, "fallback") hands
// back "name?fallback=…", which the icon provider can't parse, so a missing
// icon showed as a pink and black box. This checks first, tries the usual
// name variants, then falls back to a generic icon (or "", which draws
// nothing, never a broken texture).
Singleton {
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
