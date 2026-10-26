pragma Singleton
import QtQuick
import Quickshell
import qs.common
import Quickshell.Io

// Chat with Claude (default), Gemini, or any OpenAI-compatible endpoint
// (OpenAI, OpenRouter, Ollama). Streams over XMLHttpRequest, so API keys stay
// in-process. `chat` drives the UI; `history` is what goes back to the API.
Singleton {
    id: root

    readonly property var builtinModels: [
        { id: "claude-opus-5-5", name: "Claude Opus 5.5", provider: "anthropic" },
        { id: "claude-sonnet-5-5", name: "Claude Sonnet 5.5", provider: "anthropic" },
        { id: "claude-haiku-5-5", name: "Claude Haiku 5.5", provider: "anthropic" },
        { id: "gemini-2.5-flash", name: "Gemini 2.5 Flash", provider: "gemini" },
        { id: "llama3.2", name: "Llama 3.2 (Ollama)", provider: "ollama" }
    ]
    readonly property var models: builtinModels.concat(Config.options.ai.extraModels)
    readonly property var model: models.find(m => m.id === Config.options.ai.model) ?? models[0]
    readonly property bool needsKey: !AiKeys.has(model.provider)
    readonly property string providerName: ({ anthropic: "Anthropic", gemini: "Google AI Studio", openai: "OpenAI", openrouter: "OpenRouter", ollama: "Ollama" })[model.provider] ?? model.provider

    // UI rows: { role: "user"|"assistant"|"note", text, thinking, status, model, image (preview path) }
    property alias chat: chatModel
    ListModel { id: chatModel }

    // API-facing turns: { role, text, image?, blocks? } (image = { data, mediaType } base64;
    // blocks = Claude content blocks, echoed back unchanged)
    property var history: []
    // Image to send with the next message (Ask about the screen): { path, data, mediaType }
    property var attachment: null
    property var xhr: null
    readonly property bool busy: xhr !== null

    function setModel(id) {
        Config.options.ai.model = id;
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
        const turns = history.map(h => h.image ? { role: h.role, text: "[A screenshot was attached here]\n" + h.text } : h);
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
        const t = text.trim() || (attachment ? "What's in this screenshot?" : "");
        if (t === "" || busy)
            return;
        if (t === "/clear")
            return clear();
        if (t.startsWith("/model ")) {
            const id = t.slice(7).trim();
            const m = models.find(x => x.id === id || x.name.toLowerCase() === id.toLowerCase());
            return m ? (setModel(m.id), note("Switched to " + m.name + ".")) : note("No model called “" + id + "”. Pick one from the model menu.");
        }
        if (t.startsWith("/key ")) {
            AiKeys.set(model.provider, t.slice(5));
            return note(providerName + " key saved.");
        }
        if (needsKey)
            return note("Add your " + providerName + " API key first: type /key followed by the key.");
        send(t);
    }

    function attach(image) {
        attachment = image;
    }

    function send(text) {
        const img = attachment;
        attachment = null;
        const turn = { role: "user", text: text };
        if (img)
            turn.image = { data: img.data, mediaType: img.mediaType };
        history = history.concat([turn]);
        chatModel.append({ role: "user", text: text, thinking: "", status: "done", model: "", image: img?.path ?? "" });
        chatModel.append({ role: "assistant", text: "", thinking: "", status: "streaming", model: model.name, image: "" });

        const req = buildRequest(model, history, {});
        const x = new XMLHttpRequest();
        xhr = x;
        parser.reset(model.provider);
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

    // Message content with an optional image, per wire format
    function content(h, provider) {
        if (provider === "anthropic")
            return h.blocks ?? (h.image ? [{ type: "image", source: { type: "base64", media_type: h.image.mediaType, data: h.image.data } }, { type: "text", text: h.text }] : h.text);
        if (provider === "gemini")
            return (h.image ? [{ inline_data: { mime_type: h.image.mediaType, data: h.image.data } }] : []).concat([{ text: h.text }]);
        return h.image ? [{ type: "image_url", image_url: { url: "data:" + h.image.mediaType + ";base64," + h.image.data } }, { type: "text", text: h.text }] : h.text;
    }

    // opts: { system, effort, maxTokens, thinking (bool) } override the chat settings
    function buildRequest(m, turns, opts) {
        const sys = opts.system ?? Config.options.ai.systemPrompt;
        if (m.provider === "anthropic") {
            const body = {
                model: m.id,
                max_tokens: opts.maxTokens ?? 64000,
                stream: true,
                system: sys,
                thinking: { type: "adaptive", display: (opts.thinking ?? Config.options.ai.showThinking) ? "summarized" : "omitted" },
                output_config: { effort: opts.effort ?? Config.options.ai.effort },
                messages: turns.map(h => ({ role: h.role, content: content(h, "anthropic") }))
            };
            const headers = { "content-type": "application/json", "x-api-key": AiKeys.get("anthropic"), "anthropic-version": "2026-10-21" };
            // Server-side refusal fallback (not offered for Haiku)
            if (!m.id.includes("haiku")) {
                body.fallbacks = "default";
                headers["anthropic-beta"] = "server-side-fallback-2026-07-01";
            }
            return { url: "https://api.anthropic.com/v1/messages", headers: headers, body: body };
        }
        if (m.provider === "gemini") {
            return {
                url: "https://generativelanguage.googleapis.com/v1beta/models/" + m.id + ":streamGenerateContent?alt=sse",
                headers: { "content-type": "application/json", "x-goog-api-key": AiKeys.get("gemini") },
                body: {
                    systemInstruction: { parts: [{ text: sys }] },
                    contents: turns.map(h => ({ role: h.role === "assistant" ? "model" : "user", parts: content(h, "gemini") }))
                }
            };
        }
        // OpenAI-compatible
        const base = m.endpoint ?? (m.provider === "ollama" ? Config.options.ai.ollamaUrl + "/v1" : m.provider === "openrouter" ? "https://openrouter.ai/api/v1" : "https://api.openai.com/v1");
        const headers = { "content-type": "application/json" };
        if (m.provider !== "ollama")
            headers["authorization"] = "Bearer " + AiKeys.get(m.provider);
        return {
            url: base + "/chat/completions",
            headers: headers,
            body: { model: m.id, stream: true, messages: [{ role: "system", content: sys }].concat(turns.map(h => ({ role: h.role, content: content(h, m.provider) }))) }
        };
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
        return status === 0 ? "Couldn't reach " + p + ". Check your internet connection" + (model.provider === "ollama" ? " or that Ollama is running" : "") + "." : status === 401 || status === 403 ? "Your " + p + " API key was rejected. Type /key and paste a valid key." : status === 402 ? "There's a billing problem on your " + p + " account." : status === 404 ? "The model “" + model.id + "” isn't available. Pick another from the model menu." : status === 429 ? "Rate limited by " + p + ". Wait a moment and try again." : status === 529 || status >= 500 ? p + " is having trouble right now. Try again in a moment." : (msg || "Request failed (" + status + ").");
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
    readonly property string quickPrompt: "You answer quick questions typed into a desktop search bar. Reply in at most three short sentences or a short list. No preamble, no follow-up questions. Use Markdown sparingly."

    function quickAsk(question) {
        stopQuick();
        quick.question = question;
        quick.answer = "";
        quick.error = "";
        if (needsKey) {
            quick.error = "Add your " + providerName + " API key in the sidebar first (Super+A).";
            return;
        }
        quick.busy = true;
        const req = buildRequest(model, [{ role: "user", text: question }], { system: quickPrompt, effort: "low", maxTokens: 2048, thinking: false });
        const x = new XMLHttpRequest();
        quickXhr = x;
        quickParser.reset(model.provider);
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
