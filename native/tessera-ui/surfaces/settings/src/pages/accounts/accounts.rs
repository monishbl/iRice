//! Accounts (pages/accounts.qml): Google (google.qml), iCloud (icloud.qml)
//! and Microsoft 365 (microsoft.qml) as its parts, each with its drive as a
//! folder where it has one (DriveGroup.qml).

use super::super::super::model::{btn, Ctx, Opt, Page, Row};
use super::{ago, arr, feed, field, s};
use serde_json::{json, Value};

pub fn page(cx: &mut Ctx, p: &mut Page) {
    p.part(cx, "Google", google);
    p.part(cx, "iCloud", icloud);
    p.part(cx, "Microsoft 365", microsoft);
}

/// What the Google helper is doing, in words (google.qml's `working`)
pub fn working(op: &str, goa_waiting: bool) -> &'static str {
    match op {
        "status" => "Checking your account…",
        "sync" => "Syncing Calendar and Tasks…",
        "signin" if goa_waiting => "Connecting your account…",
        "signin" | "connect" => "Sign in to Google in your browser, then come back here…",
        "connectDesktop" => "Connecting your account…",
        "configure" => "Importing the client…",
        "disconnect" => "Disconnecting…",
        "forget" => "Removing Tessera's copy of your account data…",
        "eventAdd" | "eventInsert" => "Adding the event…",
        "eventPatch" => "Saving the event…",
        "eventDelete" => "Deleting the event…",
        "taskAdd" => "Adding the task…",
        "taskUpdate" => "Saving the task…",
        "taskDelete" => "Deleting the task…",
        "flush" => "Sending your changes to Google…",
        _ => "Working…",
    }
}

/// Buttons that each run a request of the shell's: (button, what, args)
fn buttons(cx: &mut Ctx, list: Vec<(Opt, &'static str, Value)>) -> (Vec<Opt>, String) {
    let opts: Vec<Opt> = list.iter().map(|b| b.0.clone()).collect();
    let calls: Vec<(&'static str, Value)> = list.into_iter().map(|b| (b.1, b.2)).collect();
    let id = cx.on(move |cx, a| {
        if let Some((what, args)) = calls.get(a.i.max(0) as usize) {
            cx.ask("accounts", what, args.clone());
        }
    });
    (opts, id)
}

fn google(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let g = &f["google"];
    let now = crate::util::now_ms();
    let connected = g["connected"].as_bool().unwrap_or(false);
    let busy = g["busy"].as_bool().unwrap_or(false);
    let op = s(&g["operation"]);
    let accounts = arr(&g["accounts"]);
    // Google accounts in Online Accounts not connected here yet
    let goa = arr(&g["goaAccounts"]);
    let can_sign_in = g["signin"].as_bool().unwrap_or(false);
    let chosen = {
        let c = cx.local_s("acct.google.chosen");
        if goa.iter().any(|a| s(&a["id"]) == c) { c } else { goa.first().map(|a| s(&a["id"])).unwrap_or_default() }
    };
    let quick = {
        let q = cx.local_s("acct.google.quick");
        if accounts.iter().any(|a| s(&a["id"]) == q) { q } else { accounts.first().map(|a| s(&a["id"])).unwrap_or_default() }
    };
    let last_error = s(&g["lastError"]);
    let outbox = g["outbox"].as_i64().unwrap_or(0);

    let footer = if !last_error.is_empty() {
        last_error
    } else if busy {
        working(&op, !goa.is_empty()).to_string()
    } else if outbox > 0 {
        format!("{} waiting to reach Google. They're sent when you're back online.", if outbox == 1 { "1 change is".to_string() } else { format!("{outbox} changes are") })
    } else if connected {
        "Calendar and Tasks sync when you sign in, unlock, or open Notification Center, and every 15 minutes while you use the computer. Only changes are fetched.".into()
    } else if can_sign_in || !goa.is_empty() {
        "Your password stays with Google: Tessera uses the account through GNOME Online Accounts and never sees it.".into()
    } else {
        "Connecting from here needs Tessera's native helpers (Software Update builds them). Until then, add your Google account in Online Accounts, then come back.".into()
    };

    let mut rows = Vec::new();
    for a in &accounts {
        let id = s(&a["id"]);
        let syncing = busy && op == "sync" && s(&g["pendingAccount"]) == id;
        let (opts, act) = buttons(
            cx,
            vec![
                (btn("Sync now").on(syncing).off(busy), "googleRun", json!([{"cmd": "sync", "account": id}])),
                (btn("Disconnect").danger().off(busy), "googleRun", json!([{"cmd": "disconnect", "account": id}])),
            ],
        );
        let identity = s(&a["identity"]);
        rows.push(
            Row::new(if identity.is_empty() { "Google account".to_string() } else { identity })
                .detail(if busy && op == "sync" { "Syncing…".to_string() } else { format!("Last synced {}", ago(a["syncedAt"].as_f64().unwrap_or(0.0), now)) })
                .buttons(opts, act),
        );
    }
    // Connecting one (another one, when some are connected)
    let detail = if goa.len() == 1 {
        format!("Signed in on this computer as {}", s(&goa[0]["name"]))
    } else if goa.len() > 1 {
        "Choose which account to use".into()
    } else if connected {
        "Calendars and task lists from every account show together".into()
    } else {
        "Calendar and Tasks from your Google account, kept in sync".into()
    };
    let several = goa.len() > 1;
    let ch = chosen.clone();
    let connect = cx.on(move |cx, a| {
        if a.i == 0 {
            if several {
                cx.ask("accounts", "googleRun", json!([{"cmd": "connectDesktop", "account": ch}]));
            } else {
                cx.ask("accounts", "googleConnect", Value::Null);
            }
        } else {
            cx.ask("accounts", "onlineAccounts", Value::Null);
        }
    });
    let mut opts = Vec::new();
    if !goa.is_empty() || can_sign_in {
        let mut b = btn(if connected { "Add account" } else { "Connect Google account" }).on(busy && (op == "signin" || op == "connectDesktop")).off(busy);
        if !connected {
            b = b.primary();
        }
        opts.push(b);
    } else {
        // (index 1: Online Accounts)
        opts.push(btn("Open Online Accounts"));
    }
    let connect_row = if !goa.is_empty() || can_sign_in {
        Row::new(if connected { "Add another account" } else { "Google account" }).detail(detail).buttons(opts, connect)
    } else {
        let id = cx.on(|cx, _| cx.ask("accounts", "onlineAccounts", Value::Null));
        Row::new(if connected { "Add another account" } else { "Google account" }).detail(detail).buttons(opts, id)
    };
    rows.push(connect_row);
    if several {
        let names: Vec<String> = goa.iter().map(|a| s(&a["name"])).collect();
        let ids: Vec<String> = goa.iter().map(|a| s(&a["id"])).collect();
        let cur = ids.iter().position(|x| *x == chosen).map_or(-1, |i| i as i32);
        let pick = cx.on(move |cx, a| {
            if let Some(id) = ids.get(a.i.max(0) as usize) {
                cx.local_set("acct.google.chosen", id.clone());
            }
        });
        rows.push(Row::new("Account").choice(&names, cur, pick).width(260.0));
    }
    rows.push(
        Row::new("Not signed in yet")
            .visible(!connected && goa.is_empty() && can_sign_in)
            .detail("Your browser opens at Google's sign-in. Approve access to Calendar and Tasks, then come back here."),
    );
    let gr = p.group(if accounts.len() > 1 { "Google accounts" } else { "Google account" });
    gr.footer(footer);
    for r in rows {
        gr.row(r);
    }

    // Which calendars show and where new tasks go: set on the Calendar page
    if connected {
        let n_cal = arr(&g["calendars"]).len();
        let text = cx.local_s("acct.google.event");
        let qa = quick.clone();
        // (typing keeps the text; its button, `i` 10, adds the event)
        let ev_id = cx.on(move |cx, a| {
            if a.i >= 10 {
                let text = cx.local_s("acct.google.event");
                if text.trim().is_empty() {
                    return;
                }
                cx.ask("accounts", "googleEvent", json!([qa, text]));
                cx.local_set("acct.google.event", "");
            } else {
                cx.local_set("acct.google.event", a.s.clone());
            }
        });
        let mut ev = Row::new("Add to primary calendar")
            .detail("For example: Lunch tomorrow at 1pm")
            .ctl("accounts", "field", ev_id)
            .text_value(text.clone())
            .placeholder("Event and time")
            .width(200.0);
        ev.opts = vec![btn("Add event").off(text.trim().is_empty())];
        let link = cx.link("calendar", "From your accounts");
        let g2 = p.group("Calendars");
        g2.footer(if n_cal == 0 { "No calendars yet. They appear after the first sync." } else { "Open an event in Notification Center's calendar to change it, or drag it to another day." });
        g2.row(Row::link("Shown calendars", "calendar").icon("calendar-blank").detail("Choose them on the Calendar page").target("From your accounts").id(link));
        g2.row(ev);

        let link = cx.link("calendar", "To-dos");
        let tasks = cx.on(|cx, _| cx.ask("accounts", "googleTasks", Value::Null));
        p.group("Tasks")
            .footer("Google tasks appear with your local reminders. Changes show at once and reach Google in the background; deleting can be undone for 8 seconds.")
            .row(Row::link("Where new tasks go", "calendar").icon("list-checks").detail("Choose a list on the Calendar page").target("To-dos").id(link))
            .row(Row::new("Task lists").buttons(vec![btn("Open Google Tasks")], tasks));
    }

    let keep = cx.on(|cx, _| cx.ask("accounts", "googleKeep", Value::Null));
    p.group("Keep")
        .footer("Google doesn't offer personal Keep notes to other apps, so Keep opens in your browser. Quick Note keeps your own notes on this computer.")
        .row(Row::new("Google Keep").buttons(vec![btn("Open Google Keep")], keep));

    // Google Drive as a folder (rclone)
    drive_group(cx, p, "gdrive");

    // ── More options ──
    p.more(cx);
    {
        let names: Vec<String> = accounts.iter().map(|a| {
            let i = s(&a["identity"]);
            if i.is_empty() { "Google account".to_string() } else { i }
        }).collect();
        let ids: Vec<String> = accounts.iter().map(|a| s(&a["id"])).collect();
        let cur = ids.iter().position(|x| *x == quick).map_or(-1, |i| i as i32);
        let pick = cx.on(move |cx, a| {
            if let Some(id) = ids.get(a.i.max(0) as usize) {
                cx.local_set("acct.google.quick", id.clone());
            }
        });
        p.group("Calendars").row(Row::new("Quick add uses").visible(accounts.len() > 1).choice(&names, cur, pick).width(260.0));

        // Advanced: your own OAuth client instead of Online Accounts
        let advanced = cx.local_b("acct.google.advanced");
        let adv = cx.on(|cx, _| {
            let v = cx.local_b("acct.google.advanced");
            cx.local_set("acct.google.advanced", !v);
        });
        let configured = g["configured"].as_bool().unwrap_or(false);
        let client = cx.on(|cx, a| {
            if a.i == 2 {
                cx.choose_file(&a.id, false, "Choose a file", "~/Downloads", "OAuth client|*.json");
            } else if a.i == 1 {
                let home = std::env::var("HOME").unwrap_or_default();
                let t = a.s.trim();
                let path = if let Some(rest) = t.strip_prefix('~') { format!("{home}{rest}") } else { t.to_string() };
                if !path.is_empty() {
                    cx.ask("accounts", "googleRun", json!([{"cmd": "configure", "path": path}]));
                }
            }
        });
        let (copts, cid) = buttons(cx, vec![(btn("Connect Google account").off(busy || !configured), "googleRun", json!([{"cmd": "connect"}]))]);
        let mut rows = vec![
            Row::new("Use your own OAuth client").detail("Instead of GNOME Online Accounts").switch(advanced, adv),
            Row::new("Client file").visible(advanced).detail(if configured { "Imported" } else { "The JSON file Google Cloud gives you" }).path("", "client_secret_….json", client),
            Row::new("Connect with your client").visible(advanced).buttons(copts, cid),
        ];
        for a in &accounts {
            let id = s(&a["id"]);
            let (o, i) = buttons(cx, vec![(btn("Remove local data").off(busy), "googleRun", json!([{"cmd": "forget", "account": id}]))]);
            let label = if accounts.len() > 1 { format!("Remove Tessera's copy of {}", s(&a["identity"])) } else { "Remove Tessera's copy".into() };
            rows.push(Row::new(label).detail("Deletes the synced data and unsent changes on this computer without contacting Google (for when you're offline)").buttons(o, i));
        }
        let ga = p.group("Advanced");
        ga.footer(if advanced { "A Desktop app OAuth client from your own Google Cloud project, with the Calendar and Tasks APIs enabled. Tessera keeps its tokens privately on this computer." } else { "" });
        for r in rows {
            ga.row(r);
        }
    }
    p.end_more();
}

fn needs<'a>(f: &'a Value, k: &str) -> bool {
    f["needs"][k].as_bool().unwrap_or(k != "checked" && k != "installing" && k != "building" && k != "updatesRunning")
}

fn icloud(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let c = &f["cloud"];
    let now = crate::util::now_ms();
    let busy = c["busy"].as_bool().unwrap_or(false);
    let here = c["signingIn"].as_bool().unwrap_or(false) && s(&c["signingInto"]) == "icloud";
    let mine: Vec<Value> = arr(&c["accounts"]).into_iter().filter(|a| s(&a["provider"]) == "icloud").collect();
    let err = s(&c["lastError"]);
    let footer = if !err.is_empty() && !busy && s(&c["errorFrom"]) != "microsoft" {
        err
    } else if here {
        "Checking your Apple ID…".into()
    } else if !mine.is_empty() {
        "Calendars sync when you sign in, unlock or open Notification Center, and every 15 minutes while you use the computer. Your app-specific password is kept in your keyring.".into()
    } else {
        "iCloud needs an app-specific password: Apple's way to let an app outside Apple's in, without your real password. Making one needs two-factor authentication on your Apple ID.".into()
    };
    let mut rows = Vec::new();
    for a in &mine {
        let id = s(&a["id"]);
        let (o, i) = buttons(cx, vec![(btn("Sync now").off(busy), "cloudSync", Value::Null), (btn("Sign out").danger().off(busy), "cloudDisconnect", json!([id]))]);
        let detail = if busy { "Syncing…".to_string() } else { format!("Last synced {} · {} contacts", ago(a["syncedAt"].as_f64().unwrap_or(0.0), now), a["contacts"].as_i64().unwrap_or(0)) };
        rows.push(Row::new(s(&a["identity"])).detail(detail).buttons(o, i));
    }
    let none = mine.is_empty();
    // A keyring keeps the password (the Secret Service)
    let installing = needs(f, "installing");
    let (o, i) = buttons(cx, vec![(btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing).off(needs(f, "updatesRunning")), "needsInstall", json!([["gnome-keyring", "libsecret"]]))]);
    rows.push(
        Row::new("A keyring is needed first")
            .visible(none && needs(f, "checked") && (!needs(f, "secretTool") || !needs(f, "keyring")))
            .detail("Your app-specific password is kept in a keyring, never in a file. Install GNOME Keyring (KDE's KWallet works too, with its Secret Service on). It asks for your password once.")
            .buttons(o, i),
    );
    if none {
        let (o, i) = buttons(cx, vec![(btn("Open account.apple.com").icon("arrow-square-out"), "appleAccount", Value::Null)]);
        rows.push(Row::new("1. Make an app-specific password").detail("At account.apple.com: Sign-In and Security → App-Specific Passwords → add one named Tessera. It looks like abcd-efgh-ijkl-mnop.").buttons(o, i));
        rows.push(field(cx, "acct.icloud.id", "2. Apple ID", "you@icloud.com", 260.0, false, None));
        rows.push(field(cx, "acct.icloud.pw", "3. App-specific password", "abcd-efgh-ijkl-mnop", 260.0, true, None));
        let ready = !cx.local_s("acct.icloud.id").trim().is_empty() && !cx.local_s("acct.icloud.pw").trim().is_empty();
        let sign = cx.on(|cx, _| {
            let (id, pw) = (cx.local_s("acct.icloud.id"), cx.local_s("acct.icloud.pw"));
            if id.trim().is_empty() || pw.trim().is_empty() {
                return;
            }
            cx.ask("accounts", "icloudSignIn", json!([id.trim(), pw]));
            cx.local_set("acct.icloud.pw", ""); // (not kept here once it's sent)
        });
        rows.push(
            Row::new("Sign in to iCloud")
                .detail("Calendars and contacts. Your real Apple ID password is never asked for.")
                .buttons(vec![btn(if here { "Signing in…" } else { "Sign in" }).primary().on(here).off(busy || !ready)], sign),
        );
    }
    let g = p.group("iCloud account");
    g.footer(footer);
    for r in rows {
        g.row(r);
    }
    if !none {
        let link = cx.link("calendar", "From your accounts");
        p.group("Calendars")
            .footer("Add an event from Notification Center's calendar and pick an iCloud calendar for it. Repeating events are changed on your iPhone or at icloud.com.")
            .row(Row::link("Shown calendars", "calendar").icon("calendar-blank").detail("Choose them on the Calendar page").target("From your accounts").id(link));
    }
}

fn microsoft(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let c = &f["cloud"];
    let now = crate::util::now_ms();
    let busy = c["busy"].as_bool().unwrap_or(false);
    let here = c["signingIn"].as_bool().unwrap_or(false) && s(&c["signingInto"]) == "microsoft";
    let mine: Vec<Value> = arr(&c["accounts"]).into_iter().filter(|a| s(&a["provider"]) == "microsoft").collect();
    let err = s(&c["lastError"]);
    let footer = if !err.is_empty() && !busy && s(&c["errorFrom"]) != "icloud" {
        err
    } else if here {
        "Sign in to Microsoft in your browser, then come back here…".into()
    } else if !mine.is_empty() {
        "Outlook calendars and To Do sync when you sign in, unlock or open Notification Center, and every 15 minutes while you use the computer.".into()
    } else {
        "Your password stays with Microsoft: Tessera uses the account through GNOME Online Accounts and never sees it. Work or school accounts may need their admin's approval.".into()
    };
    let mut rows = Vec::new();
    for a in &mine {
        let id = s(&a["id"]);
        let (o, i) = buttons(cx, vec![(btn("Sync now").off(busy), "cloudSync", Value::Null), (btn("Disconnect").danger().off(busy), "cloudDisconnect", json!([id]))]);
        let identity = s(&a["identity"]);
        rows.push(
            Row::new(if identity.is_empty() { "Microsoft account".to_string() } else { identity })
                .detail(if busy { "Syncing…".to_string() } else { format!("Last synced {}", ago(a["syncedAt"].as_f64().unwrap_or(0.0), now)) })
                .buttons(o, i),
        );
    }
    let none = mine.is_empty();
    let checked = needs(f, "checked");
    let (goa, goa_ms, signin) = (needs(f, "goa"), needs(f, "goaMicrosoft"), needs(f, "goaSignin"));
    let installing = needs(f, "installing");
    let (o, i) = buttons(cx, vec![(btn(if installing { "Installing…" } else if !goa { "Install" } else { "Update" }).icon("download-simple").on(installing).off(needs(f, "updatesRunning")), "needsInstall", json!([["gnome-online-accounts"]]))]);
    rows.push(
        Row::new(if !goa { "GNOME Online Accounts is needed" } else { "GNOME Online Accounts is too old" })
            .visible(none && checked && (!goa || !goa_ms))
            .detail("Microsoft sign-in goes through GNOME Online Accounts (3.50 or newer), so Tessera never sees your password. It asks for your password once.")
            .buttons(o, i),
    );
    let building = needs(f, "building");
    let build_error = s(&f["needs"]["buildError"]);
    let (o, i) = buttons(cx, vec![(btn(if building { "Building…" } else { "Build It" }).on(building), "needsBuild", Value::Null)]);
    rows.push(
        Row::new(if building { "Building Tessera's sign-in helper…" } else { "Tessera's sign-in helper isn't built yet" })
            .visible(none && checked && goa && goa_ms && !signin)
            .detail(if build_error.is_empty() { "It's built on this computer, with Tessera's other native helpers (a minute or two).".to_string() } else { build_error.clone() })
            .tone(if build_error.is_empty() { 0 } else { 2 })
            .buttons(o, i),
    );
    let (o, i) = buttons(cx, vec![(btn(if here { "Waiting for the browser…" } else { "Sign in with Microsoft" }).primary().on(here).off(busy), "microsoftSignIn", Value::Null)]);
    rows.push(
        Row::new("Microsoft 365 account")
            .visible(none && (!checked || (goa && goa_ms && signin)))
            .detail("Outlook calendars and Microsoft To Do, kept in sync. Personal (Outlook.com) and work or school accounts.")
            .buttons(o, i),
    );
    let g = p.group("Microsoft account");
    g.footer(footer);
    for r in rows {
        g.row(r);
    }
    if !none {
        let l1 = cx.link("calendar", "From your accounts");
        let l2 = cx.link("calendar", "To-dos");
        p.group("Calendars and To Do")
            .footer("Repeating events are changed in Outlook. Microsoft To Do tasks appear in To-do with your other reminders; deleting can be undone for 8 seconds.")
            .row(Row::link("Shown calendars", "calendar").icon("calendar-blank").detail("Choose them on the Calendar page").target("From your accounts").id(l1))
            .row(Row::link("Where new to-dos go", "calendar").icon("list-checks").detail("Choose a list on the Calendar page").target("To-dos").id(l2));
    }
    // OneDrive as a folder (rclone)
    drive_group(cx, p, "onedrive");
}

/// A cloud drive as a folder (DriveGroup.qml): gdrive | onedrive
fn drive_group(cx: &mut Ctx, p: &mut Page, drive: &str) {
    let f = feed(cx);
    let d = &f["drives"];
    let name = if drive == "gdrive" { "Google Drive" } else { "OneDrive" };
    let info = &d[drive];
    let connected = info["connected"].as_bool().unwrap_or(false);
    let mounted = info["mounted"].as_bool().unwrap_or(false);
    let folder = s(&info["folder"]);
    let busy = s(&d["busy"]);
    let working = busy.ends_with(&format!(" {drive}"));
    let error = s(&d["errors"][drive]);
    let has_rclone = d["hasRclone"].as_bool().unwrap_or(false);
    let footer = if !error.is_empty() {
        error
    } else if busy == format!("connect {drive}") {
        "Sign in in your browser and allow access, then come back here…".into()
    } else if connected {
        "Files open on demand from the cloud; ones you use are kept on this computer (up to 2 GB) and changes upload in the background. It comes back at every login.".into()
    } else {
        format!("{name} as a folder in Files, file pickers and the terminal. It signs in separately from the account above (through rclone).")
    };
    let mut rows = Vec::new();
    let install_error = s(&d["installError"]);
    let installing = d["installing"].as_bool().unwrap_or(false);
    let (o, i) = buttons(cx, vec![(btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing).off(needs(f, "updatesRunning")), "driveInstall", Value::Null)]);
    rows.push(
        Row::new(format!("{name} as a folder"))
            .visible(!has_rclone)
            .detail(if install_error.is_empty() { "Needs rclone. It asks for your password once.".to_string() } else { install_error.clone() })
            .tone(if install_error.is_empty() { 0 } else { 2 })
            .buttons(o, i),
    );
    if has_rclone {
        let free = busy.is_empty();
        let mut list: Vec<(Opt, &'static str, Value)> = Vec::new();
        if connected && mounted {
            list.push((btn("Open").icon("folder-open"), "driveOpen", json!([drive])));
        }
        if connected && !mounted {
            list.push((btn("Mount").on(working).off(!free), "driveAct", json!(["mount", drive])));
        }
        if connected {
            list.push((btn("Disconnect").danger().off(!free), "driveAct", json!(["disconnect", drive])));
        } else {
            list.push((btn(if working { "Waiting for the browser…".to_string() } else { format!("Connect {name}") }).on(working).off(!free), "driveAct", json!(["connect", drive])));
        }
        if busy == format!("connect {drive}") {
            list.push((btn("Cancel"), "driveCancel", Value::Null));
        }
        let (o, i) = buttons(cx, list);
        let label = if connected { format!("~/{}", folder.rsplit('/').next().unwrap_or("")) } else { format!("{name} as a folder") };
        let detail = if working { "Working…" } else if connected { if mounted { "Connected" } else { "Connected · not mounted" } } else { "Not connected" };
        rows.push(Row::new(label).detail(detail).buttons(o, i));
    }
    let g = p.group(format!("{name} folder"));
    g.footer(footer);
    for r in rows {
        g.row(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_in_words() {
        assert_eq!(working("sync", false), "Syncing Calendar and Tasks…");
        assert_eq!(working("signin", true), "Connecting your account…");
        assert_eq!(working("signin", false), "Sign in to Google in your browser, then come back here…");
        assert_eq!(working("nothing", false), "Working…");
    }
}
