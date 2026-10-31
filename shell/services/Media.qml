pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Mpris

// The player worth showing: the one you picked, else the newest one that's
// playing, else the most recent.
Singleton {
    id: root

    property var pinned: null
    readonly property var players: Mpris.players.values
    // (Newest first: a browser that restarts a tab's media session can leave
    // the old player behind, still saying "Playing", with a picture file
    // that's been deleted. The first one playing was that ghost: no art.)
    readonly property var player: {
        if (pinned && players.includes(pinned))
            return pinned;
        for (let i = players.length - 1; i >= 0; i--)
            if (players[i].isPlaying)
                return players[i];
        return players[players.length - 1] ?? null;
    }

    // ── Album art ───────────────────────────────────────────────────────
    // A player's art, ready for an Image, from the best place that works:
    //   1. what the player says (mpris:artUrl)
    //   2. what it said earlier for this track: browsers drop the art from
    //      a video's details now and then (a pause, a seek) and send none
    //   3. a YouTube video's own thumbnail, from its address (xesam:url)
    // A place whose picture fails to load is skipped for this track.
    // Browsers also write the art to a file whose name they reuse (Zen/
    // Firefox: …/firefox-mpris/<pid>_0.png, the next video's art in the same
    // file), so the address alone didn't change with the track: a local file
    // gets the track in its address (a query: the file is read as it is),
    // and is read again a moment after a track change and when it fails (it
    // may still be being written) before it's given up on.
    property int artRevision: 0
    property var lastArt: ({}) // "title|artist" -> the address it last came with
    function trackKey(p) {
        return p?.trackTitle ? p.trackTitle + "|" + (p.trackArtist ?? "") : "";
    }
    function youtubeId(url) {
        const m = String(url).match(/^https?:\/\/(?:www\.|m\.|music\.)?(?:youtube\.com\/(?:watch\?(?:.*&)?v=|shorts\/|live\/)|youtu\.be\/)([A-Za-z0-9_-]{11})/);
        return m ? m[1] : "";
    }
    function artSources(p) {
        const out = [];
        const u = String(p?.trackArtUrl ?? "");
        if (u !== "")
            out.push(u);
        const kept = lastArt[trackKey(p)] ?? "";
        if (kept !== "" && kept !== u)
            out.push(kept);
        const yt = youtubeId(p?.metadata?.["xesam:url"] ?? "");
        if (yt !== "")
            out.push("https://i.ytimg.com/vi/" + yt + "/maxresdefault.jpg", "https://i.ytimg.com/vi/" + yt + "/hqdefault.jpg");
        return out;
    }
    function withTrack(u, p) {
        return u.startsWith("file://") ? u + "?t=" + encodeURIComponent(trackKey(p)) + "&r=" + artRevision : u;
    }
    // The shown player's places, and which one works (the probe's verdict)
    readonly property var artCandidates: artSources(player)
    readonly property string artKey: (player?.dbusName ?? "") + "\n" + trackKey(player) + "\n" + artCandidates.join(" ")
    property int artIndex: 0
    onArtKeyChanged: {
        artIndex = 0;
        artRetries = 0;
    }
    function artUrl(p) {
        if (p && p === player)
            return withTrack(artCandidates[artIndex] ?? "", p);
        return withTrack(artSources(p)[0] ?? "", p);
    }
    // Tries each place in turn with an image of its own: one that fails
    // moves to the next. (Not the art images' own errors: one whose address
    // had just moved on reported the old address's failure as the new one's,
    // and a working thumbnail was skipped.) A local file is read again a few times first (the
    // browser may still be writing it).
    property int artRetries: 0
    Image {
        id: probe
        visible: false
        asynchronous: true
        sourceSize: Qt.size(64, 64)
        source: root.withTrack(root.artCandidates[root.artIndex] ?? "", root.player)
        onStatusChanged: {
            if (status !== Image.Error)
                return;
            if (String(source).startsWith("file://") && root.artRetries < 4)
                artRetry.restart();
            else
                root.artIndex++;
        }
    }
    Timer {
        id: artRetry
        interval: 400 * Math.pow(2, root.artRetries) // (0.4 s, 0.8 s, 1.6 s…)
        onTriggered: {
            root.artRetries++;
            root.artRevision++;
        }
    }
    // A new track: a local file is read again a moment later. A browser may
    // announce the track before it has written its picture into the reused
    // file, and the old picture, read first, would stay under the new
    // track's address. Its art is kept too, for when the browser drops it.
    function rememberArt() {
        const k = trackKey(player), u = String(player?.trackArtUrl ?? "");
        if (k === "" || u === "" || lastArt[k] === u)
            return;
        const next = {};
        for (const key of Object.keys(lastArt).slice(-19)) // (the last 20 tracks)
            next[key] = lastArt[key];
        next[k] = u;
        lastArt = next;
    }
    Connections {
        target: root.player
        function onTrackTitleChanged() {
            root.rememberArt();
            artSettle.restart();
        }
        function onTrackArtUrlChanged() {
            root.rememberArt();
            artSettle.restart();
        }
    }
    onPlayerChanged: rememberArt()
    Timer {
        id: artSettle
        interval: 1500
        onTriggered: {
            if (String(root.artCandidates[root.artIndex] ?? "").startsWith("file://"))
                root.artRevision++;
        }
    }
}
