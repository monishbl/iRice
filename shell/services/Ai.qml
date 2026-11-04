pragma Singleton
import QtQuick
import Quickshell
import qs.common
import Quickshell.Io
import "AiModels.js" as ModelConfig
import "AiPrompt.js" as Prompt
import "AiChats.js" as Chats

// Models and connections are owned by the user in Settings → AI. Streams
// over XMLHttpRequest, so credentials stay in-process. `chat` drives the UI;
// `history` is what goes back to the API; `sessions` lists past chats.
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

    Component.onCompleted: { migrateModels(); migratePrompt(); startHistory(); }
    Connections {
        target: Config
        function onReadyChanged() { root.migrateModels(); root.migratePrompt(); root.startHistory(); }
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

    // UI rows: { role: "user"|"assistant"|"note", text, thinking, status, model, image (preview path),
    //   search (an answer to circle to search: its picture, for "Search the web") }
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

    // ── Chat history ───────────────────────────────────────────────────
    // Every chat is a session, saved owner-only next to UI state as
    // ai-chats/<id>.json, with a light index.json for the history list
    // (AiChats.js has the format). The open chat saves itself as it goes;
    // one nobody sent anything in isn't kept. Pictures are never stored.
    // Settings → AI → Chat history can turn saving off: then nothing is
    // written, and the chat is forgotten a minute after the sidebar closes.
    readonly property string chatsDir: UiState.dir + "/ai-chats"
    readonly property string indexPath: chatsDir + "/index.json"
    readonly property string oldChatPath: UiState.dir + "/ai-chat.json" // (one chat, before history)
    readonly property bool keepHistory: Config.options.ai.keepHistory
    property var sessions: [] // the index's entries, newest first
    property string currentId: "" // the open chat's session ("" until it's first saved)
    property var trash: [] // deleted, Undo still offered: removed when that ends (or at the next start)
    property var pendingDelete: null // { entry, wasCurrent }: what Undo brings back
    property string notice: "" // something to say in the history view (no Undo)
    property bool historyReady: false
    property bool chatChanged: false // the open chat has news its file hasn't
    property bool pendingSave: false // (asked to save before the index was read)
    property bool touched: false // (a chat was started or opened before the index was read)
    readonly property var currentEntry: sessions.find(c => c.id === currentId) ?? null
    // Content landed in the open chat from elsewhere (circle to search,
    // Spotlight's Continue in chat): the sidebar shows the chat
    signal wantChat

    function pathOf(id) {
        return Chats.validId(id) ? chatsDir + "/" + id + ".json" : "";
    }

    // The pencil, /new and /clear: a new chat; the old one stays in history
    function newChat() {
        stop();
        save();
        touched = true;
        chatModel.clear();
        history = [];
        currentId = "";
        chatChanged = false;
        writeIndex();
    }
    function clear() {
        newChat();
    }

    function savedRows() {
        const rows = [];
        for (let i = 0; i < chatModel.count; i++) {
            const r = chatModel.get(i);
            if (r.status !== "streaming")
                rows.push({ role: r.role, text: r.text, thinking: r.thinking, status: r.status, model: r.model, image: r.image, search: r.search });
        }
        return rows;
    }
    // Writes the open chat when it has news (or `force`: its title or pin
    // changed), and its line in the index
    function save(force) {
        if (!historyReady) {
            pendingSave = true;
            return;
        }
        if (!keepHistory || (!chatChanged && !force))
            return;
        const rows = savedRows();
        if (!Chats.worthKeeping(rows))
            return;
        const now = Date.now();
        if (currentId === "")
            currentId = Chats.newId(now, Math.random());
        const old = currentEntry;
        const s = Chats.session({
            id: currentId,
            title: old?.title ?? Chats.titleFrom(rows.find(r => r.role === "user").text),
            created: old?.created ?? now,
            updated: chatChanged || !old ? now : old.updated,
            model: model.key,
            modelName: model.id !== "" ? model.name : (old?.modelName ?? ""),
            pinned: old?.pinned ?? false
        }, rows, history);
        queue({ path: pathOf(currentId), payload: JSON.stringify(s) });
        sessions = Chats.upsert(sessions, Chats.entryOf(s));
        chatChanged = false;
        writeIndex();
    }
    function writeIndex() {
        if (historyReady)
            Qt.callLater(root.flushIndex); // (once, however many changes)
    }
    function flushIndex() {
        queue({ path: indexPath, payload: Chats.indexText(keepHistory ? currentId : "", sessions, trash) });
    }

    // A saved chat, as the latest write left it (one still waiting to be
    // written counts)
    function readSession(id) {
        const path = pathOf(id);
        if (path === "")
            return null;
        for (let i = jobs.length - 1; i >= 0; i--)
            if (jobs[i].path === path && jobs[i].payload !== undefined)
                return parse(jobs[i].payload);
        if (writer.running && writer.path === path && writer.payload !== "")
            return parse(writer.payload);
        return readJson(path);
    }
    function parse(text) {
        try {
            return text ? JSON.parse(text) : null;
        } catch (e) {
            return null;
        }
    }
    // (small files, read at once: no answer can arrive half-way)
    Component {
        id: reader
        FileView {
            blockAllReads: true
            printErrors: false
        }
    }
    function readText(path) {
        const f = reader.createObject(root, { path: path });
        const t = f.text();
        f.destroy();
        return t;
    }
    function readJson(path) {
        return parse(readText(path));
    }
    function fillRows(rows, at) {
        const text = v => typeof v === "string" ? v : "";
        (rows ?? []).forEach((r, i) => chatModel.insert(at + i, { role: text(r.role) || "note", text: text(r.text), thinking: text(r.thinking), status: text(r.status) || "done", model: text(r.model), image: text(r.image), search: text(r.search) }));
    }

    // Open a past chat: an answer still coming is stopped (and kept) first.
    // Its model comes back too, while it's still in the list.
    function openChat(id) {
        if (id === currentId)
            return true;
        const s = readSession(id);
        if (!s || !Array.isArray(s.rows)) {
            sessions = Chats.without(sessions, id);
            writeIndex();
            say("Couldn't open that chat: its file is missing or damaged, so it's been taken off the list.");
            return false;
        }
        stop();
        save();
        touched = true;
        chatModel.clear();
        fillRows(s.rows, 0);
        history = Array.isArray(s.history) ? s.history : [];
        currentId = id;
        chatChanged = false;
        if (typeof s.model === "string" && models.some(m => m.key === s.model))
            setModel(s.model);
        writeIndex();
        return true;
    }

    function renameChat(id, title) {
        const t = Chats.cleanTitle(title);
        if (t !== "")
            updateChat(id, { title: t });
    }
    function setPinned(id, pinned) {
        updateChat(id, { pinned: pinned });
    }
    function updateChat(id, fields) {
        if (!sessions.some(c => c.id === id))
            return;
        sessions = Chats.patch(sessions, id, fields);
        if (id === currentId)
            save(true);
        else
            queue({ patch: pathOf(id), fields: fields });
        writeIndex();
    }

    // Delete acts at once; Undo brings it back for 8 s, and only then is the
    // file removed (or at the next start, if the shell stops first). The
    // open chat clears to a new one.
    function deleteChat(id) {
        finishDelete(); // (an earlier deletion is final now)
        const wasCurrent = id === currentId;
        if (wasCurrent) {
            stop();
            save(); // (so Undo brings back all of it)
        }
        const entry = sessions.find(c => c.id === id);
        if (!entry)
            return;
        if (wasCurrent)
            discardCurrent();
        sessions = Chats.without(sessions, id);
        trash = [id];
        pendingDelete = { entry: entry, wasCurrent: wasCurrent };
        notice = "";
        deleteTimer.restart();
        writeIndex();
    }
    function undoDelete() {
        const p = pendingDelete;
        if (!p)
            return;
        deleteTimer.stop();
        pendingDelete = null;
        trash = [];
        sessions = Chats.upsert(sessions, p.entry);
        writeIndex();
        if (p.wasCurrent && chatModel.count === 0 && !busy)
            openChat(p.entry.id);
    }
    function finishDelete() {
        const p = pendingDelete;
        if (!p)
            return;
        deleteTimer.stop();
        pendingDelete = null;
        queue({ paths: [pathOf(p.entry.id)] });
        trash = [];
        writeIndex();
    }
    Timer {
        id: deleteTimer
        interval: 8000
        onTriggered: root.finishDelete()
    }
    // The open chat, gone without saving (it was deleted)
    function discardCurrent() {
        flushTimer.stop();
        if (xhr) {
            const x = xhr;
            xhr = null;
            x.abort();
        }
        chatModel.clear();
        history = [];
        currentId = "";
        chatChanged = false;
    }

    // Settings → AI → Delete all chats (it asks first: there's no Undo),
    // pinned ones too
    function deleteAllChats() {
        deleteTimer.stop();
        pendingDelete = null;
        discardCurrent();
        sessions = [];
        trash = [];
        queue({ clear: chatsDir });
        writeIndex();
    }

    // Settings → AI → Delete chats older than: by their last message, never
    // the pinned or open ones
    function pruneOld() {
        if (!historyReady)
            return;
        const drop = Chats.prune(sessions, Config.options.ai.historyDays, Date.now(), [currentId]);
        if (drop.length === 0)
            return;
        sessions = sessions.filter(c => drop.indexOf(c.id) < 0);
        queue({ paths: drop.map(pathOf) });
        writeIndex();
    }
    Connections {
        target: Config.options.ai
        function onHistoryDaysChanged() { root.pruneOld(); }
        function onKeepHistoryChanged() { root.writeIndex(); }
    }
    Timer {
        interval: 6 * 3600 * 1000
        running: root.historyReady && Config.options.ai.historyDays > 0
        repeat: true
        onTriggered: root.pruneOld()
    }
    // History off: the chat lives only while it's in use
    Timer {
        interval: 60000
        running: !root.keepHistory && Panels.open !== "sidebar" && chatModel.count > 0 && !root.busy
        onTriggered: root.newChat()
    }

    function say(text) {
        notice = text;
        noticeTimer.restart();
    }
    Timer {
        id: noticeTimer
        interval: 5000
        onTriggered: root.notice = ""
    }

    // Read the index once: chats left in the trash go now, the old single
    // chat becomes the first session, and the chat that was open comes back
    // (above anything already there: circle to search can send its first
    // message before this has run).
    function startHistory() {
        if (historyReady || !Config.ready)
            return;
        const idx = Chats.parseIndex(readText(indexPath)) ?? { current: "", trash: [], chats: [] };
        sessions = idx.chats;
        let dirty = idx.trash.length > 0;
        if (idx.trash.length)
            queue({ paths: idx.trash.filter(id => !sessions.some(c => c.id === id)).map(pathOf) });
        const old = readJson(oldChatPath);
        if (old !== null) {
            const now = Date.now();
            const s = Chats.migrate(old, Chats.newId(now, Math.random()), now);
            if (s) {
                queue({ path: pathOf(s.id), payload: JSON.stringify(s) });
                sessions = Chats.upsert(sessions, Chats.entryOf(s));
                if (idx.current === "")
                    idx.current = s.id;
            }
            queue({ paths: [oldChatPath] });
            dirty = true;
        }
        historyReady = true;
        if (keepHistory && idx.current !== "" && !touched) {
            const s = readSession(idx.current);
            if (s && Array.isArray(s.rows)) {
                fillRows(s.rows, 0);
                history = (Array.isArray(s.history) ? s.history : []).concat(history);
                currentId = idx.current;
            }
        }
        pruneOld();
        if (pendingSave || chatChanged) {
            pendingSave = false;
            save();
        }
        if (dirty)
            writeIndex();
    }
    Component.onDestruction: {
        if (trash.length)
            Quickshell.execDetached(["rm", "-f", "--"].concat(trash.map(pathOf)));
    }

    // Writes, one at a time in order: { path, payload } (owner-only, whole
    // or not at all), { patch: path, fields } (a saved chat's title or pin),
    // { paths } (removed) or { clear: folder } (every chat in it removed)
    property var jobs: []
    function queue(job) {
        if ((job.path ?? job.patch ?? "x") === "")
            return;
        jobs = jobs.concat([job]);
        runNext();
    }
    function runNext() {
        if (writer.running || jobs.length === 0)
            return;
        const j = jobs[0];
        jobs = jobs.slice(1);
        let payload = j.payload;
        let path = j.path ?? j.patch ?? "";
        if (j.patch !== undefined) {
            const s = readJson(j.patch);
            if (!s)
                return runNext();
            payload = JSON.stringify(Object.assign(s, j.fields));
        }
        writer.path = payload !== undefined ? path : "";
        writer.payload = payload ?? "";
        writer.stdinEnabled = payload !== undefined;
        writer.command = payload !== undefined ? ["sh", "-c", "umask 077; mkdir -p \"$(dirname \"$1\")\" && cat > \"$1.tmp\" && mv -f \"$1.tmp\" \"$1\"", "sh", path]
            : j.clear !== undefined ? ["sh", "-c", "rm -f -- \"$1\"/*.json \"$1\"/*.tmp", "sh", j.clear]
            : ["rm", "-f", "--"].concat(j.paths.filter(p => p !== ""));
        writer.running = true;
    }
    Process {
        id: writer
        property string path
        property string payload
        onStarted: {
            if (stdinEnabled) {
                write(payload);
                stdinEnabled = false;
            }
        }
        onExited: {
            path = "";
            payload = "";
            Qt.callLater(root.runNext);
        }
    }

    // Search through whole chats (ai-chat-search.py), for words past the
    // index's preview: the ids it found for `searchedFor`
    property var searchFound: []
    property string searchedFor: ""
    readonly property bool searching: searcher.running
    function searchChats(query) {
        const q = query.trim();
        searcher.query = q;
        if (q === "") {
            searchFound = [];
            searchedFor = "";
        } else if (searcher.running) {
            searcher.again = true;
        } else {
            searcher.running = true;
        }
    }
    Process {
        id: searcher
        property string query
        property string ran
        property bool again: false
        command: ["python3", "-I", Quickshell.shellDir + "/assets/scripts/ai-chat-search.py", root.chatsDir]
        stdinEnabled: true
        onStarted: {
            ran = query;
            write(query);
            stdinEnabled = false;
        }
        stdout: StdioCollector {
            onStreamFinished: {
                if (searcher.ran !== searcher.query)
                    return;
                root.searchFound = text.split("\n").filter(id => Chats.validId(id));
                root.searchedFor = searcher.ran;
            }
        }
        onExited: {
            stdinEnabled = true;
            if (again && query !== "") {
                again = false;
                running = true;
            }
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
        chatModel.append({ role: "note", text: text, thinking: "", status: "done", model: "", image: "", search: "" });
    }

    // Circle to search: the circled picture, asked about at once. (A model
    // that can't read pictures keeps it attached: once they've switched,
    // Send asks the same question.)
    readonly property string circleQuestion: "What is this? Identify it and tell me the most useful things about it."
    function searchPicture(picture) {
        if (busy)
            stop();
        attach(Object.assign({ lens: true }, picture));
        if (attachments.some(a => a.path === picture.path))
            submit("");
        wantChat();
    }

    // Slash commands: /new (or /clear), /model <id>, /key <key>
    function submit(text) {
        const t = text.trim() || (attachments.some(a => a.lens) ? circleQuestion : attachments.length === 1 ? "What's in this picture?" : attachments.length > 1 ? "What's in these pictures?" : "");
        if (t === "" || busy)
            return;
        if (t === "/new" || t === "/clear")
            return newChat();
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
        chatChanged = true;
        // (row image: the previews' paths, one per line)
        chatModel.append({ role: "user", text: text, thinking: "", status: "done", model: "", image: imgs.map(i => i.path).join("\n"), search: "" });
        chatModel.append({ role: "assistant", text: "", thinking: "", status: "streaming", model: model.name, image: "", search: imgs.find(i => i.lens)?.path ?? "" });

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
            save();
            return;
        }
        if (status === "error") {
            chatModel.setProperty(i, "status", "error");
            chatModel.setProperty(i, "text", parser.error);
            history = history.slice(0, -1);
            save();
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
        save();
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

    // Move the quick exchange into the sidebar chat (the open one)
    function continueQuick() {
        if (quick.answer === "" || busy)
            return;
        history = history.concat([{ role: "user", text: quick.question }, { role: "assistant", text: quick.answer }]);
        chatModel.append({ role: "user", text: quick.question, thinking: "", status: "done", model: "", image: "", search: "" });
        chatModel.append({ role: "assistant", text: quick.answer, thinking: "", status: "done", model: model.name, image: "", search: "" });
        chatChanged = true;
        save();
        wantChat();
        Panels.sidebarTab = 0;
        Panels.show("sidebar");
    }
}
