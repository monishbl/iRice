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
    property var pending: []

    function get(provider, slot) {
        const env = envNames[provider] ? Quickshell.env(envNames[provider]) : "";
        const google = provider === "aistudio" || provider === "vertex" ? Quickshell.env("GOOGLE_API_KEY") : "";
        return env || google || keys[slot] || keys[provider] || (provider === "aistudio" ? keys.gemini : "") || "";
    }

    function has(provider, slot) {
        return provider === "ollama" || get(provider, slot) !== "";
    }

    function set(provider, key) {
        const k = Object.assign({}, keys);
        k[provider] = key.trim();
        keys = k;
        edited = Object.assign({}, edited, { [provider]: true });
        pending = pending.concat([{ provider: provider, key: key.trim() }]);
        storeNext();
    }
    function storeNext() {
        if (storer.running || pending.length === 0)
            return;
        const next = pending[0];
        pending = pending.slice(1);
        storer.provider = next.provider;
        storer.key = next.key;
        storer.running = true;
    }

    // Keyring store; on failure fall back to the private file.
    Process {
        id: storer
        property string provider
        property string key
        command: ["secret-tool", "store", "--label=iRice " + provider + " API key", "application", "irice", "provider", provider]
        stdinEnabled: true
        onStarted: {
            write(key);
            stdinEnabled = false;
        }
        onExited: code => {
            stdinEnabled = true;
            if (code !== 0)
                fileWriter.save();
            Qt.callLater(root.storeNext);
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
            write(JSON.stringify(root.keys));
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
                root.keys = Object.assign({}, JSON.parse(text()), root.keys);
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
