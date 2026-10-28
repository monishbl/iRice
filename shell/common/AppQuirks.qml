pragma Singleton
import QtQuick
import Quickshell

// Apps that need a nudge to behave like the others.
//   aliases  a window class with no desktop file of its own -> the app's
//            desktop entry id, so the dock and the switcher show the right
//            name and icon (Webex's windows say "CiscoCollabHost")
//   env      extra environment when iRice starts the app. Webex bundles its
//            own Qt Wayland compositor for its embedded views; on Wayland
//            those come up as a stray window of their own, so it runs
//            through XWayland, where it behaves.
Singleton {
    id: root

    readonly property var aliases: ({
            "CiscoCollabHost": "webex",
            "ciscocollabhost": "webex",
            "Webex": "webex"
        })
    // [{ match: regexp on the desktop entry id or its command, env: [...] }]
    readonly property var envs: [
        { match: /(^webex$|\/opt\/Webex\/)/i, env: ["QT_QPA_PLATFORM=xcb"] }
    ]

    function entryFor(cls) {
        return aliases[cls] ?? "";
    }
    // Extra "NAME=value" settings to start this desktop entry with
    function envFor(entry) {
        if (!entry)
            return [];
        const id = entry.id ?? "", cmd = (entry.command ?? []).join(" ");
        for (const q of envs)
            if (q.match.test(id) || q.match.test(cmd))
                return q.env;
        return [];
    }
}
