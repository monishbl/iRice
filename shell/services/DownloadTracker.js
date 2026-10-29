.pragma library
// Browsers' partial downloads in the Downloads folder, followed from one
// look to the next (Transfers.qml). Pure logic, tested in Node
// (tests/download_tracker.test.cjs).
//
// What a look sees: [{ path, bytes }] for every *.part / *.crdownload /
// *.download file. From that:
//   - a file that hasn't grown for a while (a paused or abandoned download,
//     or Chromium's empty "Unconfirmed 1234.crdownload" waiting behind its
//     save dialog) isn't shown: a card reading "Downloading 0 B" forever
//     helps nobody
//   - a file that went away after it had grown is a download that finished
//     (renamed to its real name) or was cancelled (deleted): reported in
//     `gone`, and the caller looks for the finished file to say so
//   - Chromium names a download "Unconfirmed 1234" until the save dialog is
//     answered: shown as "Download" until it has its real name

// A download that hasn't grown for this long isn't shown
var STALL_MS = 20000;

function partialName(path) {
    return String(path).split("/").pop().replace(/^\./, "").replace(/\.(part|crdownload|download)$/, "");
}
function isUnconfirmed(name) {
    return /^Unconfirmed \d+$/.test(name);
}
function displayName(path) {
    var name = partialName(path);
    return isUnconfirmed(name) ? "Download" : name;
}

// tracked: path -> { bytes, at, changedAt, grew } from the last look
// entries: this look; now: ms
// -> { tracked, items (to show), gone (went away after growing), anyShown }
function update(tracked, entries, now) {
    tracked = tracked || {};
    var next = {}, items = [], gone = [];
    for (var i = 0; i < entries.length; i++) {
        var e = entries[i];
        var last = tracked[e.path];
        var grew = !!last && e.bytes > last.bytes;
        var t = {
            bytes: e.bytes,
            at: now,
            changedAt: !last || grew ? now : last.changedAt,
            grew: (last && last.grew) || grew || (!last && e.bytes > 0)
        };
        next[e.path] = t;
        var speed = last && now > last.at ? Math.max(0, (e.bytes - last.bytes) * 1000 / (now - last.at)) : 0;
        if (e.bytes > 0 && now - t.changedAt < STALL_MS)
            items.push({ id: e.path, kind: "download", name: displayName(e.path), done: e.bytes, total: 0, speed: speed });
    }
    for (var path in tracked) {
        if (next[path])
            continue;
        var old = tracked[path];
        if (old.grew && old.bytes > 0)
            gone.push({ path: path, name: partialName(path), unconfirmed: isUnconfirmed(partialName(path)), bytes: old.bytes, at: old.changedAt });
    }
    return { tracked: next, items: items, gone: gone };
}

// The finished file for a download that went away: the same name without
// the partial suffix, else (Chromium's "Unconfirmed" ones, or a browser that
// renamed it) the newest file changed since it was last seen growing that's
// at least as big. files: [{ path, bytes, mtime (ms) }] of recent files.
// "" when nothing fits (it was cancelled).
function finalFile(goneItem, files) {
    var dir = goneItem.path.replace(/\/[^/]*$/, "");
    var best = null;
    for (var i = 0; i < files.length; i++) {
        var f = files[i];
        if (/\.(part|crdownload|download)$/.test(f.path))
            continue;
        if (!goneItem.unconfirmed && f.path === dir + "/" + goneItem.name)
            return f.path;
        if (f.bytes >= goneItem.bytes && f.mtime >= goneItem.at - 2000 && (!best || f.mtime > best.mtime))
            best = f;
    }
    return best ? best.path : "";
}
