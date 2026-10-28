pragma Singleton
import QtQuick
import Quickshell

// Starts apps and opens files and links. Normally that's the same as starting
// them directly; in the tty session (dev.sh tty, IRICE_TTY=1) the shell runs
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
    function run(cmd) {
        Quickshell.execDetached(wrap(cmd));
    }
    // A link, file or folder in its default app
    function open(url) {
        const u = url.toString();
        if (!tty) {
            Qt.openUrlExternally(u);
            return;
        }
        run(["xdg-open", u]);
    }
    // A command in a terminal: kitty, else foot, else the system's terminal.
    // hold: keep the window open after the command ends (to read its output)
    function terminal(args, hold) {
        run(["sh", "-c", "if command -v kitty >/dev/null; then exec kitty " + (hold ? "--hold " : "") + "\"$@\"; elif command -v foot >/dev/null; then exec foot " + (hold ? "-H " : "") + "\"$@\"; else exec xdg-terminal-exec \"$@\"; fi", "sh"].concat(args));
    }
    // An app from its desktop entry
    function app(entry) {
        if (!entry)
            return;
        const env = AppQuirks.envFor(entry); // (apps that need a nudge)
        if ((!tty && env.length === 0) || !entry.command || entry.command.length === 0) {
            entry.execute();
            return;
        }
        let cmd = entry.runInTerminal ? ["sh", "-c", "exec ${TERMINAL:-kitty} -e \"$@\"", "sh"].concat(entry.command) : entry.command;
        if (env.length > 0)
            cmd = ["env"].concat(env, cmd);
        if (entry.workingDirectory)
            Quickshell.execDetached({ command: wrap(cmd), workingDirectory: entry.workingDirectory });
        else
            run(cmd);
    }
}
