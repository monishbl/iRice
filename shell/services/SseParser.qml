import QtQuick

// Incremental SSE parser for the Claude, Gemini and OpenAI-compatible stream
// formats. feed() takes the whole response so far; updated fires per chunk.
QtObject {
    signal updated
    property string provider
    property int offset: 0
    property string text: ""
    property string thinking: ""
    property var blocks: []
    property string stopReason: ""
    property string error: ""

    function reset(p) {
        provider = p;
        offset = 0;
        text = "";
        thinking = "";
        blocks = [];
        stopReason = "";
        error = "";
    }

    function feed(all) {
        const end = all.lastIndexOf("\n");
        if (end < offset)
            return;
        const chunk = all.slice(offset, end);
        offset = end + 1;
        for (const line of chunk.split("\n")) {
            if (!line.startsWith("data:"))
                continue;
            const data = line.slice(5).trim();
            if (data === "" || data === "[DONE]")
                continue;
            let ev;
            try {
                ev = JSON.parse(data);
            } catch (e) {
                continue;
            }
            handle(ev);
        }
        updated();
    }

    function handle(ev) {
        if (provider === "anthropic") {
            switch (ev.type) {
            case "content_block_start":
                blocks.push(JSON.parse(JSON.stringify(ev.content_block)));
                break;
            case "content_block_delta": {
                const b = blocks[ev.index] ?? blocks[blocks.length - 1];
                const d = ev.delta;
                if (d.type === "text_delta") {
                    b.text = (b.text ?? "") + d.text;
                    text += d.text;
                } else if (d.type === "thinking_delta") {
                    b.thinking = (b.thinking ?? "") + d.thinking;
                    thinking += d.thinking;
                } else if (d.type === "signature_delta") {
                    b.signature = d.signature;
                }
                break;
            }
            case "message_delta":
                if (ev.delta?.stop_reason)
                    stopReason = ev.delta.stop_reason;
                break;
            case "error":
                error = ev.error?.type === "overloaded_error" ? "Claude is overloaded right now. Try again in a moment." : (ev.error?.message ?? "The response was interrupted. Try again.");
                break;
            }
        } else if (provider === "gemini") {
            for (const part of ev.candidates?.[0]?.content?.parts ?? []) {
                if (part.thought)
                    thinking += part.text ?? "";
                else
                    text += part.text ?? "";
            }
            if (ev.candidates?.[0]?.finishReason === "SAFETY")
                stopReason = "refusal";
        } else {
            const d = ev.choices?.[0]?.delta ?? {};
            text += d.content ?? "";
            thinking += d.reasoning ?? d.reasoning_content ?? "";
        }
    }
}
