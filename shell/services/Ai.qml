pragma Singleton
import QtQuick
import Quickshell
import qs.common
import Quickshell.Io
import "AiModels.js" as ModelConfig
import "AiPrompt.js" as Prompt

// Models and connections are owned by the user in Settings → AI. Streams
// over XMLHttpRequest, so credentials stay in-process. `chat` drives the UI;
// `history` is what goes back to the API.
Singleton {
    id: root

    readonly property var providers: ModelConfig.providers
    readonly property var models: ModelConfig.read(Config.options.ai.models)
    readonly property var model: models.find(m => m.key === Config.options.ai.model) ?? models[0] ?? ({ key: "", id: "", name: "Add a model", provider: "", auth: "none" })
    readonly property bool configured: model.id !== ""
    readonly property string credentialProvider: ModelConfig.credentialProvider(model)
    readonly property string credentialLabel: ModelConfig.credentialLabel(model)
    readonly property bool needsKey: configured && model.auth !== "none" && !AiKeys.has(credentialProvider, model.key)
    readonly property string providerName: ModelConfig.providerName(model.provider)

    function nameForProvider(provider) { return ModelConfig.providerName(provider); }
    function normalizeModel(raw) { return ModelConfig.normalize(raw); }
    function hasCredential(m) { return m.auth === "none" || AiKeys.has(ModelConfig.credentialProvider(m), m.key); }
    function saveCredential(m, key) { AiKeys.set(m.key, key, m.name + " · " + ModelConfig.credentialLabel(m)); }

    Component.onCompleted: { migrateModels(); migratePrompt(); }
    Connections {
        target: Config
        function onReadyChanged() { root.migrateModels(); root.migratePrompt(); }
    }
    // A whole system prompt from before (it replaced iRice's own) becomes the
    // person's note, unless it was the old default
    readonly property string oldDefaultPrompt: "You are a helpful assistant in a Linux desktop sidebar (Hyprland). Be concise and direct. Use Markdown: short paragraphs, lists where they help, and fenced code blocks for commands or code."
    function migratePrompt() {
        if (!Config.ready)
            return;
        const cfg = Config.options.ai;
        const old = cfg.systemPrompt.trim();
        if (old === "")
            return;
        if (old !== oldDefaultPrompt && cfg.customInstructions.trim() === "")
            cfg.customInstructions = Prompt.clean(old);
        cfg.systemPrompt = "";
    }
    function migrateModels() {
        if (!Config.ready || Config.options.ai.modelsMigrated)
            return;
        const cfg = Config.options.ai;
        const imported = ModelConfig.migrate(cfg.model, cfg.extraModels, cfg.ollamaUrl);
        const merged = ModelConfig.read((cfg.models ?? []).concat(imported));
        const selected = merged.find(m => m.key === cfg.model || m.id === cfg.model);
        cfg.models = merged.map(m => JSON.stringify(m));
        cfg.model = selected?.key ?? merged[0]?.key ?? "";
        cfg.modelsMigrated = true;
    }
    function saveModel(raw, oldKey) {
        const m = ModelConfig.normalize(raw);
        const error = ModelConfig.validate(m);
        if (error)
            return error;
        if (models.some(x => x.key === m.key && x.key !== oldKey))
            return "That model and connection are already in your list. Edit the existing entry.";
        const old = models.find(x => x.key === oldKey);
        // Renaming a model ID on the same connection keeps its saved key.
        // A different provider, host or Vertex project needs its own credential.
        const retainedKey = old?.key !== m.key && ModelConfig.sameConnection(old, m) ? AiKeys.keys[old.key] : "";
        const list = models.filter(x => x.key !== oldKey).concat([m]);
        stop();
        stopQuick();
        Config.options.ai.models = list.map(x => JSON.stringify(x));
        Config.options.ai.modelsMigrated = true;
        Config.options.ai.model = m.key;
        if (retainedKey) {
            saveCredential(m, retainedKey);
            AiKeys.clear(old.key); // (moved to the new entry, not left behind)
        }
        return "";
    }
    // Removing keeps the credential until the Undo time is over (forget()),
    // so Undo brings the model back working. Returns what restore() needs.
    function removeModel(key) {
        if (model.key === key) {
            stop();
            stopQuick();
        }
        const removed = models.find(m => m.key === key);
        const list = models.filter(m => m.key !== key);
        const wasSelected = Config.options.ai.model === key;
        Config.options.ai.models = list.map(m => JSON.stringify(m));
        if (wasSelected)
            Config.options.ai.model = list[0]?.key ?? "";
        return removed ? { model: removed, selected: wasSelected } : null;
    }
    function restoreModel(undo) {
        if (!undo || models.some(m => m.key === undo.model.key))
            return;
        Config.options.ai.models = models.concat([undo.model]).map(m => JSON.stringify(m));
        if (undo.selected)
            Config.options.ai.model = undo.model.key;
    }
    // The removed model's credential, once it can't come back
    function forgetCredential(key) {
        if (!models.some(m => m.key === key))
            AiKeys.clear(key);
    }

    // UI rows: { role: "user"|"assistant"|"note", text, thinking, status, model, image (preview path) }
    property alias chat: chatModel
    ListModel { id: chatModel }

    // API-facing turns: { role, text, image?, blocks? } (image = { data, mediaType } base64;
    // blocks = Claude content blocks, echoed back unchanged)
    property var history: []
    // Pictures to send with the next message (up to maxImages): { path, data, mediaType }
    // (path: a small JPEG copy in the runtime folder, for the previews)
    property var attachments: []
    readonly property int maxImages: 4
    // Which of the person's models read pictures (to suggest one)
    readonly property var visionModels: models.filter(m => m.vision)
    property var xhr: null
    readonly property bool busy: xhr !== null

    function setModel(id) {
        const m = models.find(m => m.key === id || m.id === id);
        if (!m || model.key === m.key)
            return;
        stop();
        stopQuick();
        Config.options.ai.model = m.key;
    }

    function clear() {
        stop();
        chatModel.clear();
        history = [];
        save();
    }

    // The conversation survives restarts: saved owner-only next to UI state.
    readonly property string chatPath: UiState.dir + "/ai-chat.json"
    function save() {
        const rows = [];
        for (let i = 0; i < chatModel.count; i++) {
            const r = chatModel.get(i);
            if (r.status !== "streaming")
                rows.push({ role: r.role, text: r.text, thinking: r.thinking, status: r.status, model: r.model, image: r.image });
        }
        // Image data stays in memory only; a saved chat remembers that there was one
        const pics = h => (h.images ?? (h.image ? [h.image] : [])).length;
        const turns = history.map(h => pics(h) ? { role: h.role, text: "[" + (pics(h) === 1 ? "A picture was" : pics(h) + " pictures were") + " attached here]\n" + h.text } : h);
        saver.payload = JSON.stringify({ rows: rows, history: turns });
        saver.running = true;
    }
    Process {
        id: saver
        property string payload
        command: ["sh", "-c", "umask 077; cat > \"$1\"", "sh", root.chatPath]
        stdinEnabled: true
        onStarted: {
            write(payload);
            stdinEnabled = false;
        }
        onExited: stdinEnabled = true
    }
    FileView {
        path: root.chatPath
        onLoaded: {
            try {
                const j = JSON.parse(text());
                for (const r of j.rows ?? [])
                    chatModel.append(Object.assign({ image: "" }, r));
                root.history = j.history ?? [];
            } catch (e) {}
        }
    }

    function stop() {
        if (xhr) {
            const x = xhr;
            xhr = null;
            x.abort();
            finish("stopped");
        }
    }

    function note(text) {
        chatModel.append({ role: "note", text: text, thinking: "", status: "done", model: "", image: "" });
    }

    // Slash commands: /clear, /model <id>, /key <key>
    function submit(text) {
        const t = text.trim() || (attachments.length === 1 ? "What's in this picture?" : attachments.length > 1 ? "What's in these pictures?" : "");
        if (t === "" || busy)
            return;
        if (t === "/clear")
            return clear();
        if (t.startsWith("/model ")) {
            const id = t.slice(7).trim();
            const m = models.find(x => x.key === id || x.id === id || x.name.toLowerCase() === id.toLowerCase());
            return m ? (setModel(m.key), note("Switched to " + m.name + ".")) : note("No model called “" + id + "”. Add it in Settings → AI or pick one from the model menu.");
        }
        if (!configured)
            return note("Add a model and provider in Settings → AI first.");
        if (t.startsWith("/key ")) {
            saveCredential(model, t.slice(5));
            return note(providerName + " credential saved.");
        }
        if (needsKey)
            return note("Add your " + providerName + " " + credentialLabel.toLowerCase() + " in Settings → AI, or type /key followed by the credential.");
        if (attachments.length > 0 && !model.vision)
            return note(cantSee());
        send(t);
    }

    // The model can't read pictures: say which of theirs can
    function cantSee() {
        const can = visionModels.map(m => m.name);
        return model.name + " can't read pictures. " + (can.length === 0
            ? "Add a model that can (Claude, Gemini or another vision model) in Settings → AI, or remove the pictures."
            : "Switch to " + (can.length === 1 ? can[0] : can.slice(0, -1).join(", ") + " or " + can[can.length - 1]) + " in the model menu (or type /model " + can[0] + "), then send again.");
    }

    // attach(null): drop them all; attach(picture): one more, while there's room
    function attach(image) {
        if (image === null)
            attachments = [];
        else if (attachments.length < maxImages)
            attachments = attachments.concat([image]);
        else
            noteFull();
    }
    function noteFull() {
        note("A message can carry " + maxImages + " pictures. Remove one to add another.");
    }
    function detach(index) {
        attachments = attachments.filter((a, i) => i !== index);
    }
    // Pictures from files (the picker, a drop) or the clipboard, made
    // send-ready by ai-image.sh; true for a paste that had a picture
    function attachFiles(paths) {
        const room = maxImages - attachments.length;
        if (room <= 0)
            return noteFull();
        if (paths.length > room)
            note("Only the first " + room + " of those were added: a message carries " + maxImages + " pictures.");
        prepare(paths.slice(0, room));
    }
    function attachClipboard() {
        if (attachments.length >= maxImages)
            return noteFull();
        prepare(["--clipboard"]);
    }
    signal pastedText // (the clipboard had no picture: paste as usual)
    function prepare(args) {
        const p = imageJob.createObject(root, { args: args });
        p.running = true;
    }
    Component {
        id: imageJob
        Process {
            id: job
            property var args: []
            command: [Quickshell.shellDir + "/assets/scripts/ai-image.sh"].concat(args)
            stdout: StdioCollector {
                onStreamFinished: {
                    const bad = [];
                    for (const line of text.split("\n")) {
                        const tab = line.indexOf("\t");
                        if (tab < 0)
                            continue;
                        const file = line.slice(0, tab), data = line.slice(tab + 1);
                        if (file === "!")
                            bad.push(data);
                        else
                            root.attach({ path: file, data: data, mediaType: "image/jpeg" });
                    }
                    if (bad.length)
                        root.note("Couldn't add " + bad.join(", ") + ": only pictures up to 64 MB (JPEG, PNG, WebP, GIF, HEIC…) can be sent.");
                }
            }
            onExited: code => {
                if (code === 3)
                    root.pastedText();
                job.destroy();
            }
        }
    }

    function send(text) {
        const imgs = attachments;
        attachments = [];
        const turn = { role: "user", text: text };
        if (imgs.length)
            turn.images = imgs.map(i => ({ data: i.data, mediaType: i.mediaType }));
        history = history.concat([turn]);
        // (row image: the previews' paths, one per line)
        chatModel.append({ role: "user", text: text, thinking: "", status: "done", model: "", image: imgs.map(i => i.path).join("\n") });
        chatModel.append({ role: "assistant", text: "", thinking: "", status: "streaming", model: model.name, image: "" });

        const req = buildRequest(model, history, {});
        const x = new XMLHttpRequest();
        xhr = x;
        parser.reset(ModelConfig.protocol(model));
        x.onreadystatechange = () => {
            if (xhr !== x)
                return;
            if (x.readyState === XMLHttpRequest.LOADING || x.readyState === XMLHttpRequest.DONE) {
                if (x.status === 200)
                    parser.feed(x.responseText);
            }
            if (x.readyState === XMLHttpRequest.DONE) {
                xhr = null;
                if (x.status === 200)
                    finish(parser.stopReason === "refusal" ? "refused" : parser.error ? "error" : "done");
                else
                    fail(x.status, x.responseText);
            }
        };
        x.open("POST", req.url);
        for (const [k, v] of Object.entries(req.headers))
            x.setRequestHeader(k, v);
        x.send(JSON.stringify(req.body));
    }

    // Every request carries iRice's system prompt (AiPrompt.js): the
    // person's style and note go inside it, never in its place
    function buildRequest(m, turns, opts) {
        const credential = AiKeys.get(ModelConfig.credentialProvider(m), m.key);
        const o = Object.assign({}, opts);
        if (o.system === undefined)
            o.system = Prompt.system(Config.options.ai);
        return ModelConfig.buildRequest(m, turns, o, Config.options.ai, credential);
    }

    // Incremental SSE parser for all three wire formats.
    SseParser {
        id: parser
        onUpdated: flushTimer.start()
    }
    // Coalesce UI updates while streaming (Markdown re-layout per token is wasteful)
    Timer {
        id: flushTimer
        interval: 50
        onTriggered: root.flush()
    }

    function flush() {
        const i = chatModel.count - 1;
        if (i < 0 || chatModel.get(i).role !== "assistant")
            return;
        chatModel.setProperty(i, "text", parser.text);
        chatModel.setProperty(i, "thinking", parser.thinking);
    }

    // Claude: after a mid-output fallback, blocks other than text before the
    // last fallback marker must not be echoed back.
    function echoBlocks(blocks) {
        const last = blocks.map(b => b.type).lastIndexOf("fallback");
        return blocks.filter((b, i) => i >= last || b.type === "text" || b.type === "fallback");
    }

    function finish(status) {
        flushTimer.stop();
        flush();
        const i = chatModel.count - 1;
        if (status === "refused") {
            // Declined (possibly after partial output): discard the partial and the question
            chatModel.setProperty(i, "text", "");
            chatModel.setProperty(i, "status", "refused");
            history = history.slice(0, -1);
            return;
        }
        if (status === "error") {
            chatModel.setProperty(i, "status", "error");
            chatModel.setProperty(i, "text", parser.error);
            history = history.slice(0, -1);
            return;
        }
        if (parser.text === "" && status === "stopped") {
            chatModel.remove(i);
            history = history.slice(0, -1);
            return;
        }
        chatModel.setProperty(i, "status", status === "stopped" ? "stopped" : "done");
        const turn = { role: "assistant", text: parser.text };
        if (parser.provider === "anthropic" && status === "done")
            turn.blocks = echoBlocks(parser.blocks);
        history = history.concat([turn]);
        save();
    }

    // HTTP errors: say what happened and how to fix it.
    function fail(status, body) {
        const i = chatModel.count - 1;
        chatModel.setProperty(i, "status", "error");
        chatModel.setProperty(i, "text", errorText(status, body));
        history = history.slice(0, -1);
    }

    function errorText(status, body) {
        let msg = "";
        try {
            const j = JSON.parse(body);
            msg = j.error?.message ?? j[0]?.error?.message ?? "";
        } catch (e) {}
        const p = providerName;
        return status === 0 ? "Couldn't reach " + p + ". Check your internet connection" + (model.provider === "ollama" ? " or that Ollama is running" : "") + "." : status === 401 || status === 403 ? "Your " + p + " credential was rejected. Check it in Settings → AI" + (model.auth === "token" ? "; OAuth access tokens expire and must be refreshed" : "") + "." : status === 402 ? "There's a billing problem on your " + p + " account." : status === 404 ? "The model “" + model.id + "” isn't available. Check its ID, provider and connection in Settings → AI." : status === 429 ? "Rate limited by " + p + ". Wait a moment and try again." : status === 529 || status >= 500 ? p + " is having trouble right now. Try again in a moment." : (msg || "Request failed (" + status + ").");
    }

    // Quick answers for Spotlight: one turn, short, low effort, kept out of
    // the chat until "Continue in chat".
    property QtObject quick: QtObject {
        property string question
        property string answer
        property bool busy: false
        property string error
    }
    property var quickXhr: null

    function quickAsk(question) {
        stopQuick();
        quick.question = question;
        quick.answer = "";
        quick.error = "";
        if (!configured || needsKey) {
            quick.error = !configured ? "Add a model and provider in Settings → AI first." : "Add your " + providerName + " " + credentialLabel.toLowerCase() + " in Settings → AI first.";
            return;
        }
        quick.busy = true;
        const req = buildRequest(model, [{ role: "user", text: question }], { system: Prompt.system(Config.options.ai, "quick"), effort: "low", maxTokens: 2048, thinking: false });
        const x = new XMLHttpRequest();
        quickXhr = x;
        quickParser.reset(ModelConfig.protocol(model));
        x.onreadystatechange = () => {
            if (quickXhr !== x)
                return;
            if ((x.readyState === XMLHttpRequest.LOADING || x.readyState === XMLHttpRequest.DONE) && x.status === 200)
                quickParser.feed(x.responseText);
            if (x.readyState === XMLHttpRequest.DONE) {
                quickXhr = null;
                quick.busy = false;
                quick.answer = quickParser.text;
                if (x.status !== 200)
                    quick.error = errorText(x.status, x.responseText);
                else if (quickParser.stopReason === "refusal")
                    quick.error = "This question can't be answered here. Try rephrasing it.";
                else if (quickParser.error)
                    quick.error = quickParser.error;
            }
        };
        x.open("POST", req.url);
        for (const [k, v] of Object.entries(req.headers))
            x.setRequestHeader(k, v);
        x.send(JSON.stringify(req.body));
    }
    SseParser {
        id: quickParser
        onUpdated: root.quick.answer = text
    }

    function stopQuick() {
        if (quickXhr) {
            const x = quickXhr;
            quickXhr = null;
            x.abort();
        }
        quick.busy = false;
    }

    // Move the quick exchange into the sidebar chat
    function continueQuick() {
        if (quick.answer === "" || busy)
            return;
        history = history.concat([{ role: "user", text: quick.question }, { role: "assistant", text: quick.answer }]);
        chatModel.append({ role: "user", text: quick.question, thinking: "", status: "done", model: "", image: "" });
        chatModel.append({ role: "assistant", text: quick.answer, thinking: "", status: "done", model: model.name, image: "" });
        save();
        Panels.sidebarTab = 0;
        Panels.show("sidebar");
    }
}
