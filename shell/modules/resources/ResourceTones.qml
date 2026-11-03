pragma Singleton
import QtQuick
import Quickshell

// The Material look's colour for a stat's tone (Resources.stats names a
// macOS accent): one of Material's three accent roles, so the graphs follow
// the wallpaper's scheme. Network's two lines get different roles.
Singleton {
    readonly property var roles: ({ blue: "primary", indigo: "primary", pink: "primary", green: "tertiary", teal: "tertiary", purple: "secondary", orange: "secondary" })
    function role(tone) {
        return roles[tone] ?? "primary";
    }
}
