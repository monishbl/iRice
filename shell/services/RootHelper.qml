pragma Singleton
import QtQuick
import Quickshell

// Tessera's root helper, as the shell runs it: through pkexec, which asks
// for the password or not as its polkit rule says. The helper is
// /usr/lib/tessera/tessera-root; until the root setup has installed it
// (`sudo python3 -I shell/assets/scripts/tessera-root.py setup shell`, or the
// next Software Update), the one installed while Tessera was called iRice,
// /usr/lib/irice/irice-root, with its own polkit actions (org.irice.root.*).
// Which one is decided when the command runs, so a setup in between never
// leaves the shell pointing at a helper that's gone.
Singleton {
    id: root

    readonly property string path: "/usr/lib/tessera/tessera-root"
    readonly property string oldPath: "/usr/lib/irice/irice-root"
    // sh: the helper that's there (Tessera's, else iRice's) as $h, and its
    // polkit actions file as $policy
    readonly property string pick: "h=" + path + "; policy=/usr/share/polkit-1/actions/org.tessera.root.policy; "
        + "if [ ! -x \"$h\" ] && [ -x " + oldPath + " ]; then h=" + oldPath + "; policy=/usr/share/polkit-1/actions/org.irice.root.policy; fi; "

    // ["pkexec", <helper>, args…], as one command for a Process
    function command(args) {
        return ["sh", "-c", pick + "exec pkexec \"$h\" \"$@\"", "sh"].concat(args);
    }
    // Is the helper installed, and does its policy have this subcommand's
    // action (an older helper may not)? As a shell test, for `sh -c`
    function knows(subcommand) {
        return pick + "[ -x \"$h\" ] && grep -q \"\\.root\\." + subcommand + "\\\"\" \"$policy\" 2>/dev/null";
    }
    // A message from the helper, without its "tessera-root: " (or the older
    // "irice-root: ") prefix
    function message(text) {
        return String(text ?? "").replace(/^(tessera|irice)-root: /, "");
    }
}
