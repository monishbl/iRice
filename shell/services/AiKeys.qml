pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// API keys for the AI sidebar. Lookup order: environment variable, system
// keyring (secret-tool), then a private 0600 file when no keyring is running.
// Keys stay in memory and are only ever sent in request headers.
Singleton {
    id: root

    readonly property var envNames: ({
            "anthropic": "ANTHROPIC_API_KEY",
            "gemini": "GEMINI_API_KEY",
            "aistudio": "GEMINI_API_KEY",
            "vertex": "VERTEX_API_KEY",
            "vertex-oauth": "GOOGLE_OAUTH_ACCESS_TOKEN",
            "openai": "OPENAI_API_KEY",
            "openrouter": "OPENROUTER_API_KEY"
        })
    readonly property string fallbackPath: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice/ai-keys.json"

    property var keys: ({})
    property var edited: ({})
    // The fallback file's own contents: only these are ever written back to
    // it (keys that live in the keyring must not be copied into a file)
    property var fileKeys: ({})
    property var pending: [] // [{ op: "store" | "clear", slot, key, label }]

    function get(provider, slot) {
        const env = envNames[provider] ? Quickshell.env(envNames[provider]) : "";
        const google = provider === "aistudio" || provider === "vertex" ? Quickshell.env("GOOGLE_API_KEY") : "";
        return env || google || keys[slot] || keys[provider] || (provider === "aistudio" ? keys.gemini : "") || "";
    }

    function has(provider, slot) {
        return provider === "ollama" || get(provider, slot) !== "";
    }

    // label: what the keyring shows for it, e.g. "Claude · API key"
    function set(slot, key, label) {
        const k = Object.assign({}, keys);
        k[slot] = key.trim();
        keys = k;
        edited = Object.assign({}, edited, { [slot]: true });
        pending = pending.concat([{ op: "store", slot: slot, key: key.trim(), label: label || slot }]);
        next();
    }
    // Forget a credential everywhere (a removed model's): memory, the keyring
    // and the fallback file
    function clear(slot) {
        if (!slot)
            return;
        const k = Object.assign({}, keys);
        delete k[slot];
        keys = k;
        edited = Object.assign({}, edited, { [slot]: true });
        if (fileKeys[slot] !== undefined) {
            const f = Object.assign({}, fileKeys);
            delete f[slot];
            fileKeys = f;
            fileWriter.save();
        }
        pending = pending.concat([{ op: "clear", slot: slot }]);
        next();
    }
    function next() {
        if (keyring.running || pending.length === 0)
            return;
        const job = pending[0];
        pending = pending.slice(1);
        keyring.job = job;
        keyring.command = job.op === "clear" ? ["secret-tool", "clear", "application", "irice", "provider", job.slot]
            : ["secret-tool", "store", "--label=iRice · " + job.label, "application", "irice", "provider", job.slot];
        keyring.stdinEnabled = job.op === "store";
        keyring.running = true;
    }

    // Keyring store or clear; a store that fails falls back to the private
    // file. The key goes in on stdin, never on the command line.
    Process {
        id: keyring
        property var job: ({})
        onStarted: {
            if (job.op === "store") {
                write(job.key);
                stdinEnabled = false;
            }
        }
        onExited: code => {
            if (code !== 0 && job.op === "store") {
                root.fileKeys = Object.assign({}, root.fileKeys, { [job.slot]: job.key });
                fileWriter.save();
            }
            Qt.callLater(root.next);
        }
    }

    Process {
        id: fileWriter
        property bool again: false
        function save() {
            if (running) {
                again = true;
                return;
            }
            command = ["sh", "-c", "umask 077; mkdir -p \"$(dirname \"$1\")\"; cat > \"$1\"", "sh", root.fallbackPath];
            running = true;
        }
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.fileKeys));
            stdinEnabled = false;
        }
        onExited: {
            stdinEnabled = true;
            if (again) {
                again = false;
                Qt.callLater(save);
            }
        }
    }

    // Load: file first (if any), then keyring entries override.
    FileView {
        path: root.fallbackPath
        onLoaded: {
            try {
                root.fileKeys = JSON.parse(text()) ?? {};
                root.keys = Object.assign({}, root.fileKeys, root.keys);
            } catch (e) {}
        }
    }

    readonly property var slots: {
        const ids = ["anthropic", "gemini", "aistudio", "vertex", "vertex-oauth", "openai", "openrouter"];
        for (const entry of Config.options.ai.models ?? []) {
            try {
                const key = JSON.parse(entry).key;
                if (key && !ids.includes(key))
                    ids.push(key);
            } catch (e) {}
        }
        return ids;
    }
    Instantiator {
        model: root.slots
        delegate: Process {
            id: lookup
            required property string modelData
            running: true
            command: ["secret-tool", "lookup", "application", "irice", "provider", lookup.modelData]
            stdout: StdioCollector {
                onStreamFinished: {
                    if (text.trim() === "" || root.edited[lookup.modelData])
                        return;
                    const k = Object.assign({}, root.keys);
                    k[lookup.modelData] = text.trim();
                    root.keys = k;
                }
            }
        }
    }
}
