pragma Singleton
import QtQuick
import Quickshell

// What an app icon looks like, measured once per icon at 32×32 (by
// IconAnalyzer, which lives in the wallpaper window): does it already fill a
// square/rounded tile, what colour is that tile, and its dominant colour.
// get(source) returns { square, bg, dominant } or null until measured.
Singleton {
    id: root

    property var info: ({})
    property var queue: []
    property var pending: ({})

    function get(source) {
        if (!source)
            return null;
        const i = info[source];
        if (i === undefined && !pending[source])
            Qt.callLater(enqueue, source);
        return i ?? null;
    }

    function enqueue(source) {
        if (info[source] !== undefined || pending[source])
            return;
        pending[source] = true;
        queue = queue.concat([source]);
    }

    // Called by the analyzer
    function next() {
        return queue.length > 0 ? queue[0] : "";
    }
    function done(source, result) {
        const i = Object.assign({}, info);
        i[source] = result;
        info = i;
        delete pending[source];
        queue = queue.filter(q => q !== source);
    }
}
