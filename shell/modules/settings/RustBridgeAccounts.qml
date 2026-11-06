import QtQuick
import Quickshell
import qs.common
import qs.services

// Settings drawn in Rust: the "accounts" page group's side in this shell
// (RustBridge.qml): Accounts (Google, iCloud, Microsoft 365, their drives),
// Calendar, AI & Sidebar, Default Apps. While Settings is open (`live`) it
// tells the pages what only this shell knows (`feed`, the pages'
// cx.feed["accounts"]) and does what they ask (`act`: cx.ask("accounts",
// what, args)): signing in and out, syncing, the AI models' records and
// their credentials (the keyring is this shell's: a typed key comes here
// once and is stored; no key is ever sent back, only whether one is set).
Scope {
    id: root

    property bool live: false

    // ── AI: a removed model can come back for 8 s (its credential is
    // forgotten after that), as pages/ai.qml ────────────────────────────
    property var aiUndo: null // { text, removed }
    Timer {
        id: aiUndoTimer
        interval: 8000
        onTriggered: root.finishAiUndo()
    }
    function finishAiUndo() {
        if (aiUndo?.removed)
            Ai.forgetCredential(aiUndo.removed.model.key);
        aiUndo = null;
    }

    // (whether a credential is set for each saved model: true or false only)
    function aiHasKey(m) {
        return Ai.hasCredential(m);
    }
    function envKeys() {
        const out = [];
        for (const p in AiKeys.envNames)
            if (Quickshell.env(AiKeys.envNames[p]))
                out.push(p);
        if (Quickshell.env("GOOGLE_API_KEY"))
            out.push("aistudio", "vertex");
        return out;
    }

    readonly property var feed: live ? ({
            signingIn: Google.signingIn,
            google: {
                connected: Google.connected,
                configured: Google.configured,
                busy: Google.busy,
                operation: Google.operation,
                pendingAccount: Google.pending?.account ?? "",
                lastError: Google.lastError,
                outbox: Google.outbox,
                signin: Google.account.signin === true,
                goaSettings: !!Google.account.goaSettings,
                goaAccounts: (Google.account.goaAccounts ?? []).filter(a => !a.used).map(a => ({ id: a.id, name: a.name ?? "" })),
                accounts: Google.accounts.map(a => ({ id: a.id, identity: a.identity ?? "", syncedAt: a.syncedAt ?? 0 })),
                calendars: Google.calendars.map(c => ({ account: c.account ?? "", id: c.id ?? "", title: c.title ?? "", primary: !!c.primary, writable: !!c.writable })),
                taskLists: Google.taskLists.map(l => ({ account: l.account ?? "", id: l.id ?? "", title: l.title ?? "" }))
            },
            cloud: {
                busy: CloudAccounts.busy,
                signingIn: CloudAccounts.signingIn,
                signingInto: CloudAccounts.signingInto,
                lastError: CloudAccounts.lastError,
                errorFrom: CloudAccounts.errorFrom,
                accounts: CloudAccounts.accounts.map(a => ({ id: a.id, provider: a.provider ?? "", identity: a.identity ?? "", syncedAt: a.syncedAt ?? 0, contacts: CloudAccounts.contacts.filter(c => c.account === a.id).length })),
                calendars: CloudAccounts.calendars.map(c => ({ account: c.account ?? "", id: c.id ?? "", title: c.title ?? "", writable: !!c.writable })),
                taskLists: CloudAccounts.taskLists.map(l => ({ account: l.account ?? "", id: l.id ?? "", title: l.title ?? "" }))
            },
            needs: {
                checked: Needs.checked,
                secretTool: Needs.secretTool,
                keyring: Needs.keyring,
                goa: Needs.goa,
                goaMicrosoft: Needs.goaMicrosoft,
                goaSignin: Needs.goaSignin,
                installing: Needs.installing,
                building: Needs.building,
                buildError: Needs.buildError,
                updatesRunning: Updates.running
            },
            drives: {
                hasRclone: CloudDrives.hasRclone,
                busy: CloudDrives.busy,
                installing: CloudDrives.installing,
                installError: CloudDrives.installError,
                errors: CloudDrives.errors,
                gdrive: CloudDrives.drive("gdrive"),
                onedrive: CloudDrives.drive("onedrive")
            },
            calendar: {
                sources: Calendar.sources.map(s => ({ name: s.name ?? "", url: s.url ?? "", color: String(s.color ?? "#8E8E93"), enabled: s.enabled !== false })),
                lastError: Calendar.lastError,
                defaultAlert: Calendar.defaultAlert
            },
            ai: {
                providers: Ai.providers,
                hasKey: Ai.models.reduce((o, m) => (o[m.key] = root.aiHasKey(m), o), {}),
                envKeys: root.envKeys(),
                sessions: Ai.sessions.length,
                pinned: Ai.sessions.filter(c => c.pinned).length,
                undo: root.aiUndo?.text ?? ""
            },
            tools: ["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm", "zen-browser", "firefox", "google-chrome-stable", "chromium", "brave", "code", "codium", "zed", "cursor", "windsurf", "gnome-text-editor", "kate", "mousepad", "pavucontrol", "pavucontrol-qt", "gnome-system-monitor", "plasma-systemmonitor", "btop", "htop"].filter(t => Tools.has(t))
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { accounts: feed } })
    onLiveChanged: if (!live) finishAiUndo()

    function act(what, args) {
        const a = args ?? [];
        switch (what) {
        // Google
        case "googleRun": // [{ cmd, account?, path? }]: sync, disconnect, connectDesktop, connect, configure, forget
            if (a[0] && ["sync", "disconnect", "connectDesktop", "connect", "configure", "forget"].includes(a[0].cmd))
                Google.run(a[0]);
            break;
        case "googleConnect":
            Google.connect();
            break;
        case "googleEvent": // [account, text]
            if (String(a[1] ?? "").trim() !== "")
                Google.write({ cmd: "eventAdd", account: String(a[0] ?? ""), calendarId: "primary", text: String(a[1]) });
            break;
        case "googleTasks":
            Launch.open("https://tasks.google.com/");
            break;
        case "googleKeep":
            Google.openKeep();
            break;
        case "onlineAccounts": {
            const app = Google.account.goaSettings;
            if (!app) {
                Google.lastError = "Install gnome-online-accounts and gnome-control-center (or gnome-online-accounts-gtk), then try again.";
                return;
            }
            Launch.run(app.endsWith("gnome-control-center") ? ["env", "XDG_CURRENT_DESKTOP=GNOME", app, "online-accounts", "google"] : [app]);
            break;
        }
        // iCloud and Microsoft 365
        case "cloudSync":
            CloudAccounts.sync();
            break;
        case "cloudDisconnect":
            CloudAccounts.disconnect(String(a[0] ?? ""));
            break;
        case "icloudSignIn": // [Apple ID, app-specific password] (sent once, kept nowhere here)
            if (String(a[0] ?? "").trim() !== "" && String(a[1] ?? "").trim() !== "")
                CloudAccounts.connectICloud(String(a[0]).trim(), String(a[1]));
            break;
        case "microsoftSignIn":
            CloudAccounts.connectMicrosoft();
            break;
        case "appleAccount":
            Launch.open("https://account.apple.com/account/manage");
            break;
        case "needsInstall":
            if (Array.isArray(a[0]))
                Needs.install(a[0].map(String));
            break;
        case "needsBuild":
            Needs.buildNative();
            break;
        // Drives
        case "driveInstall":
            CloudDrives.install();
            break;
        case "driveOpen":
            CloudDrives.open(String(a[0] ?? ""));
            break;
        case "driveAct": // [mount | disconnect | connect, drive]
            if (["mount", "disconnect", "connect"].includes(a[0]))
                CloudDrives.act(a[0], String(a[1] ?? ""));
            break;
        case "driveCancel":
            CloudDrives.cancel();
            break;
        // Calendar's subscriptions
        case "calSet": // [url, enabled]
            Calendar.setSource(String(a[0] ?? ""), { enabled: !!a[1] });
            break;
        case "calRemove":
            Calendar.removeSource(String(a[0] ?? ""));
            break;
        case "calAdd": // [name, url]
            Calendar.addSource(String(a[0] ?? ""), String(a[1] ?? ""));
            break;
        case "calAlert":
            Calendar.defaultAlert = Number(a[0] ?? 10);
            Calendar.saveSources();
            break;
        // AI
        case "aiSetModel":
            Ai.setModel(String(a[0] ?? ""));
            break;
        case "aiSave": { // [draft, old key, credential] (checked by the page first; the credential goes to the keyring, never back)
            const raw = a[0] ?? {};
            const error = Ai.saveModel(raw, String(a[1] ?? ""));
            if (error) {
                console.warn("settings (accounts): model not saved: " + error);
                return;
            }
            const m = Ai.normalizeModel(raw);
            const cred = String(a[2] ?? "");
            if (m.auth !== "none" && cred.trim() !== "")
                Ai.saveCredential(m, cred);
            break;
        }
        case "aiRemove": {
            root.finishAiUndo(); // (an earlier removal is final now)
            const m = Ai.models.find(x => x.key === String(a[0] ?? ""));
            if (!m)
                return;
            const removed = Ai.removeModel(m.key);
            if (removed) {
                root.aiUndo = { text: m.name + " removed", removed: removed };
                aiUndoTimer.restart();
            }
            break;
        }
        case "aiUndo":
            if (root.aiUndo) {
                Ai.restoreModel(root.aiUndo.removed);
                root.aiUndo = null;
                aiUndoTimer.stop();
            }
            break;
        case "aiDeleteAll":
            Ai.deleteAllChats();
            break;
        default:
            console.warn("settings (accounts): no act " + what);
        }
    }
}
