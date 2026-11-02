pragma Singleton
import QtQuick
import Quickshell

// Starts apps and opens files and links, each app in a scope of its own
// under the user's app.slice (assets/scripts/app-scope.sh), so an app
// working flat out shares the CPU with the desktop instead of taking it
// from Hyprland and this shell (without systemd: started directly). In the tty session (dev.sh tty, IRICE_TTY=1) the shell runs
// with sandbox config, data and state folders, and anything it starts would
// inherit them: apps would miss their settings and open links with whatever
// registered itself in the sandbox. There, those variables are dropped first,
// so apps get the user's real folders and default apps.
Singleton {
    id: root

    readonly property bool tty: Quickshell.env("IRICE_TTY") === "1"
    readonly property var scrub: tty ? ["env", "-u", "XDG_CONFIG_HOME", "-u", "XDG_DATA_HOME", "-u", "XDG_STATE_HOME", "-u", "XDG_CACHE_HOME", "-u", "IRICE_SANDBOX", "-u", "IRICE_ROOT", "-u", "IRICE_TTY", "-u", "QML_IMPORT_PATH", "-u", "QML2_IMPORT_PATH"] : []

    // A command line for something that starts apps
    function wrap(cmd) {
        return scrub.concat(cmd);
    }
    readonly property string appScope: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/app-scope.sh").toString().replace("file://", ""))
    // An app's command line, in a scope of its own (`id`: what it's named after)
    function scoped(cmd, id) {
        return [appScope, "--id", id ?? "", "--"].concat(wrap(cmd));
    }
    function run(cmd, id) {
        Quickshell.execDetached(scoped(cmd, id));
    }
    // A link, file or folder in its default app
    function open(url) {
        run(["xdg-open", url.toString()], "open");
    }
    // A command in a terminal: the one chosen in Settings → Default apps,
    // else $TERMINAL, else the first installed (kitty, foot, alacritty,
    // wezterm, ghostty, konsole, …), each started its own way
    // (assets/scripts/open-app.sh). hold: keep the window open after the
    // command ends (to read its output)
    readonly property string openApp: decodeURIComponent(Qt.resolvedUrl("../assets/scripts/open-app.sh").toString().replace("file://", ""))
    function terminalCommand(args, hold) {
        return ["env", "IRICE_USER_DIR=" + Config.dir, "bash", openApp, "terminal"].concat(hold ? ["--hold"] : [], ["--"], args);
    }
    function terminal(args, hold) {
        run(terminalCommand(args, hold), "terminal");
    }
    // An app from its desktop entry
    function app(entry) {
        if (!entry)
            return;
        const env = AppQuirks.envFor(entry); // (apps that need a nudge)
        if (!entry.command || entry.command.length === 0) {
            entry.execute();
            return;
        }
        let cmd = entry.runInTerminal ? terminalCommand(entry.command, false) : entry.command;
        if (env.length > 0)
            cmd = ["env"].concat(env, cmd);
        if (entry.workingDirectory)
            Quickshell.execDetached({ command: scoped(cmd, entry.id), workingDirectory: entry.workingDirectory });
        else
            run(cmd, entry.id);
    }
}
