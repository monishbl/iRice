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
    // A player's art, ready for an Image. Browsers write it to a file whose
    // name they reuse (Zen/Firefox: …/firefox-mpris/<pid>_0.png, the next
    // video's art in the same file), so the address alone didn't change with
    // the track: the old picture stayed (Qt's cache), or one read while the
    // file was being written failed and was never tried again. A local file
    // gets the track in its address (a query: the file is read as it is), and
    // an image that fails asks again a little later (artFailed).
    property int artRevision: 0
    function artUrl(p) {
        const u = String(p?.trackArtUrl ?? "");
        if (!u.startsWith("file://"))
            return u;
        return u + "?t=" + encodeURIComponent((p?.trackTitle ?? "") + "|" + (p?.trackArtist ?? "")) + "&r=" + artRevision;
    }
    property int artRetries: 0
    function artFailed() {
        if (artRetries < 5 && !artRetry.running)
            artRetry.start();
    }
    Timer {
        id: artRetry
        interval: 400 * Math.pow(2, root.artRetries) // (0.4 s, 0.8 s, 1.6 s…: the file is being written)
        onTriggered: {
            root.artRetries++;
            root.artRevision++;
        }
    }
    // A new track: read again a moment later too. A browser may announce the
    // track before it has written its picture into the reused file, and the
    // old picture, read first, would stay under the new track's address.
    Connections {
        target: root.player
        function onTrackTitleChanged() {
            artSettle.restart();
        }
        function onTrackArtUrlChanged() {
            artSettle.restart();
        }
    }
    Timer {
        id: artSettle
        interval: 1500
        onTriggered: {
            root.artRetries = 0;
            if (String(root.player?.trackArtUrl ?? "").startsWith("file://"))
                root.artRevision++;
        }
    }
    // (tries start over now and then: a later track gets its own)
    Timer {
        running: root.artRetries > 0
        interval: 30000
        onTriggered: root.artRetries = 0
    }
}
