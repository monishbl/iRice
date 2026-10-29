// Desktop Entry Exec tokenization. Never interpret the result through a shell.
function parse(exec, name) {
    const args = [];
    let word = "", quoted = false, escaped = false, started = false;
    for (const char of exec) {
        if (escaped) { word += char; escaped = false; started = true; }
        else if (char === "\\") { escaped = true; started = true; }
        else if (char === '"') { quoted = !quoted; started = true; }
        else if (/\s/.test(char) && !quoted) {
            if (started) args.push(word);
            word = ""; started = false;
        } else { word += char; started = true; }
    }
    if (quoted || escaped) throw Error("Invalid session command quoting");
    if (started) args.push(word);
    const result = [];
    for (const arg of args) {
        if (/^%[fFuUdDnNickvm]$/.test(arg) && arg !== "%c") continue;
        const value = arg.replace(/%(%|c|.)/g, (_, code) => {
            if (code === "%") return "%";
            if (code === "c") return name || "";
            throw Error("Unsupported session command field: %" + code);
        });
        result.push(value);
    }
    if (!result.length || !result[0]) throw Error("Empty session command");
    return result;
}
