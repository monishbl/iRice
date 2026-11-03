pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// Wi-Fi networks as QR codes (assets/scripts/wifi-qr.py), like a phone's
// "Share" on its Wi-Fi: the network you're on (or a saved one) as a code
// another phone's camera joins, with its password on request; and a Wi-Fi
// code in a screenshot offers to join that network. The code is a private
// file in the runtime folder, removed when the card closes.
Singleton {
    id: root

    readonly property string script: Quickshell.shellDir + "/assets/scripts/wifi-qr.py"

    // The card: "" | loading | ready | error
    property string stage: ""
    property string ssid: ""
    property string password: ""
    property string security: ""
    property string qr: "" // the code's file
    property string error: ""

    property int request: 0 // (an answer to an older one is dropped)
    function share(name) {
        close();
        ssid = name;
        stage = "loading";
        request++;
        const p = shareComp.createObject(root, { req: request, command: ["python3", "-I", script, "share", name] });
        p.running = true;
    }
    function forget(file) {
        Quickshell.execDetached(["python3", "-I", script, "forget", file]);
    }
    function close() {
        if (qr !== "")
            forget(qr);
        request++;
        stage = "";
        qr = "";
        password = ""; // (not kept once the card is gone)
        error = "";
    }
    Component {
        id: shareComp
        Process {
            id: job
            property int req
            stdout: StdioCollector {
                onStreamFinished: {
                    let r = {};
                    try {
                        r = JSON.parse(text);
                    } catch (e) {}
                    if (job.req !== root.request || root.stage !== "loading") {
                        // (closed, or another network asked for, meanwhile: its
                        // code holds the password, so it isn't left behind)
                        if (r.qr)
                            root.forget(r.qr);
                        return;
                    }
                    if (r.ok) {
                        root.qr = r.qr;
                        root.password = r.password ?? "";
                        root.security = r.security ?? "";
                        root.stage = "ready";
                    } else {
                        root.error = r.error || "The network couldn't be shared.";
                        root.stage = "error";
                    }
                }
            }
            onExited: job.destroy()
        }
    }

    // ── A Wi-Fi code in a screenshot ────────────────────────────────────
    // (ScreenTools, after each screenshot: a quick local look with zbar)
    function scan(file) {
        find(file, found => {
            for (const w of found)
                root.offer(w);
        });
    }
    // The Wi-Fi codes in a picture: done([{ ssid, security, password,
    // hidden }]) (also Live Text's chips)
    function find(file, done) {
        const p = scanComp.createObject(root, { command: ["python3", "-I", script, "scan", file], done: done });
        p.running = true;
    }
    Component {
        id: scanComp
        Process {
            id: job
            property var done
            stdout: StdioCollector {
                onStreamFinished: {
                    let found = [];
                    try {
                        found = JSON.parse(text).found ?? [];
                    } catch (e) {}
                    if (job.done)
                        job.done(found);
                }
            }
            onExited: job.destroy()
        }
    }
    // A code a computer can join from: open, or WPA with a real password
    function joinable(w) {
        return w.security === "nopass" || ((w.security === "WPA" || w.security === "SAE") && w.password.length >= 8);
    }
    function join(w) {
        WifiJoin.join({ ssid: w.ssid, hidden: w.hidden, security: w.security === "nopass" ? "open" : "psk", psk: w.password });
    }
    function offer(w) {
        const joinable = root.joinable(w);
        Island.post({
            kind: "custom", icon: "wifi-high", title: "Wi-Fi network in your screenshot",
            subtitle: w.ssid + (w.security === "nopass" ? " · open network" : ""), color: Theme.accent,
            action: joinable ? "Join" : (w.password !== "" ? "Copy Password" : ""),
            onAction: () => {
                if (joinable)
                    root.join(w);
                else
                    Clipboard.copyText(w.password);
            },
            action2: joinable && w.password !== "" ? "Copy Password" : "",
            onAction2: () => Clipboard.copyText(w.password)
        }, 12000);
    }
}
