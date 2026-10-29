import QtQuick
import QtQuick.Controls.Basic
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: root
    width: parent?.width ?? 0
    spacing: 24

    property string editingKey: ""
    property string provider: "aistudio"
    property string auth: "apiKey"
    property bool thinking: false
    property string message: ""
    property bool messageIsError: false

    // A removed model can come back for a few seconds; its credential is
    // forgotten only after that
    property var undo: null // { text, removed }
    Timer {
        id: undoTimer
        interval: 8000
        onTriggered: root.finishUndo()
    }
    function finishUndo() {
        if (undo?.removed)
            Ai.forgetCredential(undo.removed.model.key);
        undo = null;
    }
    function remove(m) {
        finishUndo(); // (an earlier removal is final now)
        const removed = Ai.removeModel(m.key);
        if (editingKey === m.key)
            edit(null);
        if (removed) {
            undo = { text: m.name + " removed", removed: removed };
            undoTimer.restart();
        }
    }

    function edit(m) {
        editingKey = m?.key ?? "";
        provider = m?.provider ?? "aistudio";
        auth = m?.auth ?? "apiKey";
        thinking = m?.thinking ?? false;
        nameField.text = m?.name ?? "";
        modelField.text = m?.id ?? "";
        endpointField.text = m?.endpoint ?? "";
        projectField.text = m?.project ?? "";
        locationField.text = m?.location || "global";
        tokensField.text = String(m?.maxTokens || 4096);
        credentialField.text = "";
        message = "";
        messageIsError = false;
    }
    function draft() {
        return { id: modelField.text, name: nameField.text, provider: provider,
            auth: auth, endpoint: endpointField.text, project: projectField.text,
            location: locationField.text, thinking: thinking, maxTokens: tokensField.text };
    }
    function save() {
        const raw = draft();
        const error = Ai.saveModel(raw, editingKey);
        message = error;
        messageIsError = error !== "";
        if (error)
            return;
        const m = Ai.normalizeModel(raw);
        if (m.auth !== "none" && credentialField.text.trim() !== "")
            Ai.saveCredential(m, credentialField.text);
        edit(m);
        message = "Model saved and selected.";
    }

    Group {
        width: parent.width
        title: "Your models"
        footer: "Add the model IDs available on your provider account. Only your saved models appear in the sidebar and Spotlight."
        Row {
            label: "Selected model"
            detail: Ai.configured ? Ai.providerName : "Add your first model below"
            Dropdown {
                width: 270
                enabled: Ai.models.length > 0
                options: Ai.models.length > 0 ? Ai.models.map(m => ({ value: m.key, label: m.name + " · " + Ai.nameForProvider(m.provider) })) : [{ value: "", label: "No models added" }]
                value: Ai.model.key
                onChosen: v => Ai.setModel(v)
            }
        }
        Repeater {
            model: Ai.models
            Row {
                id: modelRow
                required property var modelData
                label: modelData.name
                detail: Ai.nameForProvider(modelData.provider) + " · " + modelData.id
                Item {
                    width: 155
                    height: 28
                    SettingsButton {
                        text: "Edit"
                        anchors.left: parent.left
                        onClicked: root.edit(modelRow.modelData)
                    }
                    SettingsButton {
                        text: "Remove"
                        danger: true
                        anchors.right: parent.right
                        onClicked: root.remove(modelRow.modelData)
                    }
                }
            }
        }
    }

    Group {
        visible: root.undo !== null
        width: parent.width
        Row {
            label: root.undo?.text ?? ""
            SettingsButton {
                text: "Undo"
                icon: "arrow-counter-clockwise"
                onClicked: {
                    Ai.restoreModel(root.undo.removed);
                    root.undo = null;
                    undoTimer.stop();
                }
            }
        }
    }

    Group {
        width: parent.width
        title: root.editingKey ? "Edit model" : "Add model"
        footer: "Each model keeps its own credential, so a second model on the same provider needs its key pasted again. Credentials are stored in your system keyring, or a private file when no keyring is available. Provider environment variables take priority. Leave the credential field empty to keep an existing credential."
        Row {
            label: "Display name"
            detail: "Optional; defaults to the model ID"
            TextField { id: nameField; width: 270; placeholder: "Name shown in iRice" }
        }
        Row {
            label: "Provider"
            detail: root.provider === "vertex" ? "Gemini models on Vertex AI" : ""
            Dropdown {
                width: 270
                options: Ai.providers
                value: root.provider
                onChosen: v => {
                    root.provider = v;
                    root.auth = v === "ollama" ? "none" : "apiKey";
                    root.message = "";
                }
            }
        }
        Row {
            label: "Model ID"
            detail: root.provider === "openrouter" ? "Use the full provider/model ID from OpenRouter" : "Copy the exact model ID from your provider"
            TextField { id: modelField; width: 270; placeholder: "Model ID" }
        }
        Row {
            visible: root.provider === "vertex" || root.provider === "custom"
            label: "Authentication"
            Dropdown {
                width: 270
                options: root.provider === "vertex" ? [{ value: "apiKey", label: "API key (Vertex Express)" }, { value: "token", label: "OAuth access token" }] : [{ value: "apiKey", label: "API key" }, { value: "none", label: "No authentication" }]
                value: root.auth
                onChosen: v => root.auth = v
            }
        }
        Row {
            visible: root.provider === "custom" || root.provider === "ollama"
            label: "API base URL"
            detail: "Include /v1 if your server uses it; omit /chat/completions"
            TextField {
                id: endpointField
                width: 270
                placeholder: root.provider === "ollama" ? Config.options.ai.ollamaUrl + "/v1" : "https://your-server.example/v1"
            }
        }
        Row {
            visible: root.provider === "vertex" && root.auth === "token"
            label: "Google Cloud project"
            TextField { id: projectField; width: 270; placeholder: "Project ID or number" }
        }
        Row {
            visible: root.provider === "vertex" && root.auth === "token"
            label: "Location"
            detail: "A supported region, or global"
            TextField { id: locationField; width: 270; text: "global" }
        }
        Row {
            visible: root.provider !== "ollama" && root.auth !== "none"
            label: root.auth === "token" ? "OAuth access token" : "API key"
            detail: root.auth === "token" ? "Short-lived; replace it when it expires" : (Ai.hasCredential(Ai.normalizeModel(root.draft())) ? "A saved credential or environment variable is available" : "Paste a credential for this connection")
            TextField {
                id: credentialField
                width: 270
                secret: true
                commitOnBlur: false
                placeholder: "Paste or replace credential"
            }
        }
        Row {
            visible: root.provider === "anthropic"
            label: "Max output tokens"
            detail: "Must fit the selected model's output limit"
            TextField { id: tokensField; width: 120; text: "4096" }
        }
        Row {
            visible: root.provider === "anthropic"
            label: "Adaptive thinking"
            detail: "Enable only if this model supports adaptive thinking"
            Switch { checked: root.thinking; onToggled: root.thinking = !checked }
        }
        Row {
            // Status of the last save: an error says how to fix it
            label: root.message
            labelColor: root.messageIsError ? Theme.color.danger : Theme.color.secondary
            Item {
                width: 222
                height: 28
                SettingsButton {
                    anchors.left: parent.left
                    text: root.editingKey ? "Save changes" : "Add model"
                    primary: true
                    onClicked: root.save()
                }
                SettingsButton {
                    anchors.right: parent.right
                    text: "New model"
                    onClicked: root.edit(null)
                }
            }
        }
    }

    Group {
        width: parent.width
        title: "Conversation"
        Row {
            visible: Ai.model.provider === "anthropic" && Ai.model.thinking === true
            label: "Effort"
            detail: "For models with adaptive thinking enabled"
            Segmented {
                width: 300
                readonly property var values: ["low", "medium", "high", "xhigh", "max"]
                options: ["Low", "Medium", "High", "X-High", "Max"]
                current: Math.max(0, values.indexOf(Config.options.ai.effort))
                onSelected: i => Config.options.ai.effort = values[i]
            }
        }
        Row {
            label: "Show reasoning summary"
            Switch { checked: Config.options.ai.showThinking; onToggled: Config.options.ai.showThinking = !checked }
        }
        Row {
            label: "System prompt"
            detail: "Instructions included with each request"
        }
        Item {
            width: parent.width
            height: 140
            ScrollView {
                anchors { fill: parent; margins: 12 }
                TextArea {
                    wrapMode: TextEdit.Wrap
                    text: Config.options.ai.systemPrompt
                    color: Theme.color.label
                    background: null
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                    onEditingFinished: Config.options.ai.systemPrompt = text
                }
            }
        }
    }
}
