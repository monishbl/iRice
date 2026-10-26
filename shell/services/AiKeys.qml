pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// API keys for the AI sidebar. Lookup order: environment variable, system
// keyring (secret-tool), then a private 0600 file when no keyring is running.
// Keys stay in memory and are only ever sent in request headers.
Singleton {
    id: root

    readonly property var envNames: ({
            "anthropic": "ANTHROPIC_API_KEY",
            "gemini": "GEMINI_API_KEY",
            "openai": "OPENAI_API_KEY",
            "openrouter": "OPENROUTER_API_KEY"
        })
    readonly property string fallbackPath: (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/irice/ai-keys.json"

    property var keys: ({})

    function get(provider) {
        return Quickshell.env(envNames[provider] ?? "") || keys[provider] || "";
    }

    function has(provider) {
        return provider === "ollama" || get(provider) !== "";
    }

    function set(provider, key) {
        const k = Object.assign({}, keys);
        k[provider] = key.trim();
        keys = k;
        storer.provider = provider;
        storer.key = key.trim();
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
        }
    }

    Process {
        id: fileWriter
        function save() {
            command = ["sh", "-c", "umask 077; mkdir -p \"$(dirname \"$1\")\"; cat > \"$1\"", "sh", root.fallbackPath];
            running = true;
        }
        stdinEnabled: true
        onStarted: {
            write(JSON.stringify(root.keys));
            stdinEnabled = false;
        }
        onExited: stdinEnabled = true
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

    Instantiator {
        model: ["anthropic", "gemini", "openai", "openrouter"]
        delegate: Process {
            id: lookup
            required property string modelData
            running: true
            command: ["secret-tool", "lookup", "application", "irice", "provider", lookup.modelData]
            stdout: StdioCollector {
                onStreamFinished: {
                    if (text.trim() === "")
                        return;
                    const k = Object.assign({}, root.keys);
                    k[lookup.modelData] = text.trim();
                    root.keys = k;
                }
            }
        }
    }
}
