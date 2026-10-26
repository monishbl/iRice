import QtQuick
import QtQuick.Controls.Basic
import qs.common
import qs.modules.settings
import qs.services

Column {
    id: root
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        Row {
            label: "Model"
            Dropdown {
                width: 220
                options: Ai.models.map(m => ({ value: m.id, label: m.name }))
                value: Config.options.ai.model
                onChosen: v => Config.options.ai.model = v
            }
        }
        Row {
            label: "Effort"
            detail: "How hard Claude thinks. Higher is slower and costs more."
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
    }

    Group {
        width: parent.width
        title: "API keys"
        footer: "Stored in your system keyring (or a private file if no keyring is running). Environment variables such as ANTHROPIC_API_KEY take priority."
        Repeater {
            model: [
                { id: "anthropic", name: "Anthropic" },
                { id: "gemini", name: "Google AI Studio" },
                { id: "openai", name: "OpenAI" },
                { id: "openrouter", name: "OpenRouter" }
            ]
            Row {
                id: keyRow
                required property var modelData
                label: modelData.name
                detail: AiKeys.has(modelData.id) ? "Key saved" : "No key"
                TextField {
                    width: 240
                    secret: true
                    placeholder: AiKeys.has(keyRow.modelData.id) ? "Replace key" : "Paste key"
                    onCommitted: t => {
                        if (t.trim() !== "") {
                            AiKeys.set(keyRow.modelData.id, t);
                            text = "";
                        }
                    }
                }
            }
        }
        Row {
            label: "Ollama URL"
            TextField {
                width: 240
                text: Config.options.ai.ollamaUrl
                onCommitted: t => Config.options.ai.ollamaUrl = t.trim()
            }
        }
    }

    Group {
        width: parent.width
        title: "System prompt"
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
