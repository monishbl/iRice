pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Mpris

// The player worth showing: the one you picked, else one that's playing,
// else the most recent.
Singleton {
    id: root

    property var pinned: null
    readonly property var players: Mpris.players.values
    readonly property var player: (pinned && players.includes(pinned)) ? pinned : players.find(p => p.isPlaying) ?? players[players.length - 1] ?? null
}
