.pragma library
// The drop shelf's pure logic (services/Shelf.qml), tested in Node
// (tests/shelf_logic.test.cjs): what a drop adds, what a drag out carries,
// and whether the pointer was shaken.

// A file's kind on the shelf, from its name
function kindOf(path) {
    if (/\.(png|jpe?g|gif|webp|bmp|svg|avif|heic|tiff?)$/i.test(path))
        return "image";
    return /\/$/.test(path) ? "folder" : "file";
}

// The shelf after a drop: file URLs (once each), else the text. `added`
// says how many came.
function drop(items, urls, text) {
    var list = items.slice();
    var have = list.map(function (i) { return i.uri || ""; });
    var added = 0;
    (urls || []).forEach(function (u) {
        var uri = String(u);
        if (uri.indexOf("file://") !== 0 || have.indexOf(uri) >= 0)
            return;
        var path = decodeURIComponent(uri.slice(7));
        list.push({ uri: uri, path: path, name: path.replace(/\/$/, "").split("/").pop(), kind: kindOf(path) });
        have.push(uri);
        added++;
    });
    var t = String(text || "").trim();
    if (added === 0 && t !== "" && !/^file:\/\//.test(t)) {
        list.push({ text: t, name: t.replace(/\s+/g, " ").slice(0, 80), kind: "text" });
        added++;
    }
    return { items: list, added: added };
}

// What a drag out of the shelf carries: the files' URIs, and text (the
// text items, else the files' paths)
function mimeFor(list) {
    var uris = list.filter(function (i) { return i.uri; }).map(function (i) { return i.uri; });
    var texts = list.filter(function (i) { return i.kind === "text"; }).map(function (i) { return i.text; });
    var m = {};
    if (uris.length > 0)
        m["text/uri-list"] = uris.join("\r\n") + "\r\n";
    if (texts.length > 0)
        m["text/plain"] = texts.join("\n");
    else if (uris.length > 0)
        m["text/plain"] = list.filter(function (i) { return i.path; }).map(function (i) { return i.path; }).join("\n");
    return m;
}

// Shaken: four or more turns among the samples ([{t, x}], the last 0.7 s),
// each after at least `reach` px the other way
function shaken(samples, reach) {
    var min = reach || 40;
    var turns = 0, dir = 0, from = samples.length > 0 ? samples[0].x : 0;
    for (var i = 1; i < samples.length; i++) {
        var dx = samples[i].x - samples[i - 1].x;
        if (dx === 0)
            continue;
        var d = dx > 0 ? 1 : -1;
        if (dir !== 0 && d !== dir) {
            if (Math.abs(samples[i - 1].x - from) >= min)
                turns++;
            from = samples[i - 1].x;
        }
        dir = d;
    }
    return turns >= 4;
}
