pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Who's logged in, read once: the account's full name (its passwd entry's)
// and picture (~/.face, or AccountsService's). Quick Settings' footer, the
// lock screens and Today's greeting show them.
Singleton {
    id: root

    property string login: Quickshell.env("USER") ?? ""
    property string fullName: ""
    // The full name, else the login
    readonly property string name: fullName || login
    // The picture's path ("" for none) and its address for an Image
    property string picture: ""
    readonly property string pictureUrl: picture !== "" ? "file://" + picture : ""

    Process {
        running: true
        command: ["sh", "-c", "u=\"$(id -un)\"; echo \"$u\"; getent passwd \"$u\" | cut -d: -f5 | cut -d, -f1; for f in \"$HOME/.face\" \"$HOME/.face.icon\" \"/var/lib/AccountsService/icons/$u\"; do [ -s \"$f\" ] && [ -r \"$f\" ] && { echo \"$f\"; exit 0; }; done; echo"]
        stdout: StdioCollector {
            onStreamFinished: {
                const l = text.split("\n").map(s => s.trim());
                root.login = l[0] || root.login;
                root.fullName = l[1] ?? "";
                root.picture = l[2] ?? "";
            }
        }
    }
}
