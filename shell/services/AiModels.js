.pragma library

// User-owned model definitions. Provider defaults describe API formats, never
// a catalogue of model IDs. No credentials are stored in these records.
const providers = [
    { value: "aistudio", label: "Google AI Studio" },
    { value: "vertex", label: "Google Vertex AI" },
    { value: "openrouter", label: "OpenRouter" },
    { value: "anthropic", label: "Anthropic" },
    { value: "openai", label: "OpenAI" },
    { value: "ollama", label: "Ollama" },
    { value: "custom", label: "OpenAI-compatible endpoint" }
];

function providerName(provider) {
    return providers.find(p => p.value === provider)?.label ?? provider;
}

function normalize(raw) {
    const text = value => String(value ?? "").trim();
    const provider = raw.provider === "gemini" ? "aistudio" : text(raw.provider);
    const m = {
        id: text(raw.id), name: text(raw.name) || text(raw.id), provider: provider,
        endpoint: provider === "custom" || provider === "ollama" ? text(raw.endpoint).replace(/\/+$/, "") : "",
        project: provider === "vertex" ? text(raw.project) : "",
        location: provider === "vertex" ? text(raw.location) || "global" : "",
        auth: provider === "ollama" ? "none" : provider === "vertex" && raw.auth === "token" ? "token" : provider === "custom" && raw.auth === "none" ? "none" : "apiKey",
        thinking: provider === "anthropic" && raw.thinking === true,
        maxTokens: provider === "anthropic" ? Number(raw.maxTokens ?? 4096) : 0
    };
    if (m.provider === "aistudio")
        m.id = m.id.replace(/^models\//, "");
    if (m.provider === "vertex")
        m.id = m.id.replace(/^publishers\/google\/models\//, "");
    if (m.provider === "vertex" && m.auth === "apiKey") {
        m.project = "";
        m.location = "global";
    }
    m.key = [m.provider, m.id, m.endpoint, m.auth, m.project, m.location].map(encodeURIComponent).join("|");
    return m;
}

function validate(m) {
    if (!providers.some(p => p.value === m.provider))
        return "Choose a supported provider.";
    if (!m.id || /[\s?#]/.test(m.id))
        return "Enter the model ID supplied by your provider, without spaces or a URL.";
    if ((m.provider === "aistudio" || m.provider === "vertex") && !/^[a-zA-Z0-9._-]+$/.test(m.id))
        return "Enter a Google model ID, not a full endpoint URL.";
    if (m.provider === "custom" && !m.endpoint)
        return "Enter the API base URL for this endpoint.";
    if (m.endpoint && !/^https?:\/\/[^\s/?#@]+(?:\/[^\s?#]*)?$/.test(m.endpoint))
        return "Use an http:// or https:// API base URL without credentials, query parameters or a fragment.";
    if (m.provider === "vertex" && m.auth === "token") {
        if (!/^[a-zA-Z0-9._:-]+$/.test(m.project))
            return "Enter your Google Cloud project ID or project number.";
        if (!/^[a-z0-9-]+$/.test(m.location))
            return "Enter a Vertex AI region or global.";
    }
    if (m.provider === "anthropic" && (!Number.isInteger(m.maxTokens) || m.maxTokens < 1))
        return "Max output tokens must be a positive whole number.";
    return "";
}

function read(entries) {
    const out = [];
    for (const entry of entries ?? []) {
        try {
            const m = normalize(typeof entry === "string" ? JSON.parse(entry) : entry);
            if (!validate(m) && !out.some(x => x.key === m.key))
                out.push(m);
        } catch (e) {}
    }
    return out;
}

function protocol(m) {
    return m.provider === "aistudio" || m.provider === "vertex" ? "gemini" : m.provider;
}

function credentialProvider(m) {
    return m.provider === "vertex" && m.auth === "token" ? "vertex-oauth" : m.provider;
}

function credentialLabel(m) {
    return m.auth === "token" ? "OAuth access token" : "API key";
}

function sameConnection(a, b) {
    return a && b && ["provider", "endpoint", "auth", "project", "location"].every(field => a[field] === b[field]);
}

// Only for importing the single active choice from pre-editor configs. These
// IDs are never offered as defaults or presets in a new installation.
function migrate(selected, extras, ollamaUrl) {
    const oldProviders = {
        "claude-opus-5-5": "anthropic", "claude-sonnet-5-5": "anthropic",
        "claude-haiku-5-5": "anthropic", "gemini-2.5-flash": "aistudio", "llama3.2": "ollama"
    };
    const raw = (extras ?? []).map(m => Object.assign({}, m));
    if (selected && !raw.some(m => m.id === selected) && oldProviders[selected])
        raw.unshift({ id: selected, name: selected, provider: oldProviders[selected], thinking: oldProviders[selected] === "anthropic" });
    for (const m of raw) {
        if (m.provider === "anthropic") {
            m.thinking = m.thinking ?? true;
            m.maxTokens = m.maxTokens ?? 64000;
        }
        if (m.provider === "ollama" && !m.endpoint)
            m.endpoint = (ollamaUrl || "http://localhost:11434").replace(/\/+$/, "") + "/v1";
    }
    return read(raw);
}

function content(h, provider) {
    if (provider === "anthropic")
        return h.blocks ?? (h.image ? [{ type: "image", source: { type: "base64", media_type: h.image.mediaType, data: h.image.data } }, { type: "text", text: h.text }] : h.text);
    if (provider === "gemini")
        return (h.image ? [{ inline_data: { mime_type: h.image.mediaType, data: h.image.data } }] : []).concat([{ text: h.text }]);
    return h.image ? [{ type: "image_url", image_url: { url: "data:" + h.image.mediaType + ";base64," + h.image.data } }, { type: "text", text: h.text }] : h.text;
}

function buildRequest(m, turns, opts, settings, credential) {
    const error = validate(m);
    if (error)
        throw new Error(error);
    const sys = opts.system ?? settings.systemPrompt;
    const headers = { "content-type": "application/json" };
    if (m.provider === "anthropic") {
        const body = { model: m.id, max_tokens: opts.maxTokens ?? m.maxTokens, stream: true, system: sys,
            messages: turns.map(h => ({ role: h.role, content: content(h, "anthropic") })) };
        if (m.thinking && opts.thinking !== false) {
            body.thinking = { type: "adaptive", display: settings.showThinking ? "summarized" : "omitted" };
            body.output_config = { effort: opts.effort ?? settings.effort };
        }
        headers["x-api-key"] = credential;
        headers["anthropic-version"] = "2026-10-21";
        return { url: "https://api.anthropic.com/v1/messages", headers: headers, body: body };
    }
    if (m.provider === "aistudio" || m.provider === "vertex") {
        let base;
        if (m.provider === "aistudio") {
            base = "https://generativelanguage.googleapis.com/v1beta/models/";
            headers["x-goog-api-key"] = credential;
        } else if (m.auth === "apiKey") {
            base = "https://aiplatform.googleapis.com/v1/publishers/google/models/";
            headers["x-goog-api-key"] = credential;
        } else {
            const host = m.location === "global" ? "aiplatform.googleapis.com" : m.location + "-aiplatform.googleapis.com";
            base = "https://" + host + "/v1/projects/" + encodeURIComponent(m.project) + "/locations/" + encodeURIComponent(m.location) + "/publishers/google/models/";
            headers["authorization"] = "Bearer " + credential;
        }
        const body = { systemInstruction: { parts: [{ text: sys }] }, contents: turns.map(h => ({ role: h.role === "assistant" ? "model" : "user", parts: content(h, "gemini") })) };
        if (opts.maxTokens)
            body.generationConfig = { maxOutputTokens: opts.maxTokens };
        return { url: base + encodeURIComponent(m.id) + ":streamGenerateContent?alt=sse", headers: headers, body: body };
    }
    const base = m.endpoint || (m.provider === "ollama" ? settings.ollamaUrl.replace(/\/+$/, "") + "/v1" : m.provider === "openrouter" ? "https://openrouter.ai/api/v1" : "https://api.openai.com/v1");
    if (m.auth !== "none")
        headers["authorization"] = "Bearer " + credential;
    return { url: base + "/chat/completions", headers: headers,
        body: { model: m.id, stream: true, messages: [{ role: "system", content: sys }].concat(turns.map(h => ({ role: h.role, content: content(h, m.provider) }))) } };
}
