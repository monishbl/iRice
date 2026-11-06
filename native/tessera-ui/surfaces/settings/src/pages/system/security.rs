//! Privacy & Security (pages/security.qml) with Fingerprint and Face
//! (biometrics.qml, its untitled part) and the add-a-finger sheet
//! (FingerprintSheet.qml, FingerprintArt.qml: art.rs).
//!
//! What only the shell knows comes in feed.system (RustBridgeSystem.qml):
//! `fingerprint`, `sheet`, `face`, `check` (sharing-check.py's status),
//! `quickShare`, `ports`, `updatesRunning`. Everything that changes the
//! system (enrolling, removing a finger, PAM, Howdy, the firewall, Quick
//! Share's trust) is the shell's: cx.ask("system", …).
//!
//! Not as the QML: the sheet isn't its own window over the whole screen; it
//! takes the page's place (on a dimmed backdrop) while it's open, and Esc
//! closes Settings rather than the sheet (Cancel closes the sheet).

use super::super::super::model::{btn, Ctx, MenuItem, Opt, Page, Row};
use super::art;
use serde_json::{json, Value};

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// The fingers in fprintd's order, with their names (Fingerprint.fingerNames)
pub fn fingers(feed: &Value) -> Vec<(String, String)> {
    feed["fingerprint"]["fingerOrder"]
        .as_array()
        .map(|a| a.iter().filter_map(|k| k.as_str()).map(|k| (k.to_string(), s(&feed["fingerprint"]["fingerNames"][k]))).collect())
        .unwrap_or_default()
}

pub fn finger_name(feed: &Value, f: &str) -> String {
    let n = s(&feed["fingerprint"]["fingerNames"][f]);
    if n.is_empty() {
        f.to_string()
    } else {
        n
    }
}

/// The page's firewall line (security.qml's Firewall row)
pub fn firewall_detail(error: &str, kind: &str, open: usize, why: &str, asked: bool) -> String {
    if !error.is_empty() {
        error.to_string()
    } else if kind.is_empty() {
        "No firewall stands in sharing's way.".into()
    } else if open > 0 {
        format!("Your firewall ({kind}) lets Quick Share and LocalSend through now, while you're {}, and closes again after.", if why.is_empty() { "sharing" } else { why })
    } else {
        format!("Your firewall ({kind}) lets Quick Share and LocalSend through only while you're visible to others or sharing.{}", if asked { " Closed now." } else { "" })
    }
}

/// What's said under the sheet's print, and its title (FingerprintSheet.qml)
pub fn sheet_texts(done: bool, failed: bool, edges: bool, finger: &str, retry: &str, stage: &str, error: &str, target: i32) -> (String, String) {
    const STEPS: [&str; 11] = [
        "Rest the middle of your finger flat on the reader, then lift it.",
        "Again, a little higher up your finger.",
        "Again, with your finger a little to the right.",
        "Again, a little lower, toward the crease.",
        "Again, with your finger a little to the left.",
        "Touch with the tip of your finger.",
        "Roll onto the upper right edge.",
        "Now the lower right edge.",
        "Now the bottom of the print, near the crease.",
        "Now the lower left edge.",
        "And last, the upper left edge.",
    ];
    let title = if done {
        format!("Your {finger} is added")
    } else if failed {
        "That didn't work".into()
    } else if edges {
        "Now the edges".into()
    } else {
        format!("Add your {finger}")
    };
    let text = if done {
        "It unlocks the lock screen now, and password prompts where you've turned that on.".to_string()
    } else if failed {
        error.to_string()
    } else if !retry.is_empty() {
        retry.to_string()
    } else if stage == "starting" {
        "Getting the reader ready…".into()
    } else {
        STEPS.get(target.max(0) as usize).copied().unwrap_or("").to_string()
    };
    (title, text)
}

/// The add-a-finger sheet, in the page's place while it's open
fn sheet(cx: &mut Ctx, p: &mut Page, finger: &str) {
    let feed = cx.feed;
    let fp = &feed["system"]["fingerprint"];
    let sh = &feed["system"]["sheet"];
    let stage = s(&fp["enrollStage"]);
    let done = stage == "done";
    let failed = stage == "error";
    let taken = art::taken(done, fp["enrollCount"].as_i64().unwrap_or(0), fp["enrollStages"].as_i64().unwrap_or(0));
    let target: i32 = if done || failed { -1 } else { taken as i32 };
    let edges = target >= 5;
    let name = finger_name(&feed["system"], finger).to_lowercase();
    let name = if name.is_empty() { "finger".to_string() } else { name };
    let retry = s(&sh["retry"]);
    let (title, text) = sheet_texts(done, failed, edges, &name, &retry, &stage, &s(&fp["enrollError"]), target);
    let caption = if done {
        "All 11 sections recorded".to_string()
    } else if failed {
        String::new()
    } else {
        format!("Section {} of 11 · {}", target + 1, if edges { "the edges" } else { "the centre" })
    };
    let flash = sh["flash"].as_bool().unwrap_or(false);
    let mut data: Vec<Opt> = art::section_paths()
        .iter()
        .enumerate()
        .map(|(i, path)| Opt::new(path.clone()).on(i < taken).off(i as i32 == target).style(if flash && i as i32 == target { 1 } else { 0 }).num(i as f32))
        .collect();
    // (the misses and takes so far: the print shakes and bumps with each)
    data.push(Opt::new("").id("meta").num(sh["misses"].as_f64().unwrap_or(0.0) as f32).style(sh["takes"].as_i64().unwrap_or(0) as i32));

    // Buttons: Add Another Finger (done, another free), Try Again (failed), Done / Cancel
    let enrolled = strings(&fp["enrolled"]);
    let all = fingers(&feed["system"]);
    let next = all.iter().map(|f| f.0.clone()).find(|f| !enrolled.contains(f) && f != finger);
    let mut rows = Vec::new();
    if done {
        if let Some(n) = next {
            let id = cx.on(move |cx, _| {
                cx.local_set("fp.sheet", n.clone());
                cx.ask("system", "fpEnroll", json!([n.clone()]));
            });
            rows.push(Row::new("").button("Add Another Finger", id));
        }
    }
    if failed {
        let f = finger.to_string();
        let id = cx.on(move |cx, _| cx.ask("system", "fpEnroll", json!([f.clone()])));
        rows.push(Row::new("").button("Try Again", id));
    }
    let enrolling = fp["enrolling"].as_bool().unwrap_or(false);
    let close = cx.on(move |cx, _| {
        if enrolling {
            cx.ask("system", "fpCancel", Value::Null);
        }
        cx.local_set("fp.sheet", "");
    });
    let b = Row::new("").button(if done { "Done" } else { "Cancel" }, close);
    rows.push(if done { b.primary() } else { b });

    let blk = p.custom("system", "fp-sheet");
    blk.title(title).text(text).data(data);
    blk.footer = caption;
    blk.on = done;
    blk.dim = !retry.is_empty() && !done && !failed;
    blk.num = target as f32;
    for r in rows {
        blk.row(r);
    }
}

/// Fingerprint and Face (biometrics.qml)
pub fn biometrics(cx: &mut Ctx, p: &mut Page) {
    let feed = cx.feed;
    let sys = &feed["system"];
    let fp = &sys["fingerprint"];
    let confirming = cx.local_s("bio.confirming");
    let updates_running = sys["updatesRunning"].as_bool().unwrap_or(false);

    // ── Fingerprint: only on a computer with a reader ──
    if fp["hasReader"].as_bool().unwrap_or(false) {
        let ready = fp["ready"].as_bool().unwrap_or(false);
        let pam_error = s(&fp["pamError"]);
        let footer = if !ready {
            String::new()
        } else {
            format!(
                "Password prompts, sudo and the login screen wait a few seconds for a finger, then ask for your password. Changing them asks for your password.{}",
                if pam_error.is_empty() { String::new() } else { format!(" {pam_error}") }
            )
        };
        let mut rows: Vec<Row> = Vec::new();

        // 1. What the reader still needs
        if !ready {
            let missing = strings(&fp["missing"]);
            let install_error = s(&fp["installError"]);
            let fix_error = s(&fp["fixError"]);
            let fixing = fp["fixing"].as_bool().unwrap_or(false);
            let slow = fp["slowStart"].as_bool().unwrap_or(false);
            let detail = if !install_error.is_empty() {
                install_error.clone()
            } else if fp["readerUnsupported"].as_bool().unwrap_or(false) {
                "Found, but it has no Linux driver yet (libfprint). Your password works as always; fprint.freedesktop.org lists the readers that do.".into()
            } else if !missing.is_empty() {
                format!(
                    "Found. To use it, install {}{}. It asks for your password once.",
                    missing.join(", "),
                    if fp["aur"].as_bool().unwrap_or(false) { " (from the AUR: Tessera sets up an AUR helper first if there's none)" } else { "" }
                )
            } else if fixing {
                "Setting up the reader. Its firmware is updating; this takes about a minute.".into()
            } else if !fix_error.is_empty() {
                fix_error.clone()
            } else if slow {
                "Found. Its driver updates the reader's firmware the first time, which takes about a minute, and the system stops it after 15 seconds. Set Up gives it the time it needs.".into()
            } else {
                "Found, but fprintd can't use it yet. Restart, then open this page again.".into()
            };
            let name = s(&fp["readerName"]);
            let mut r = Row::new(if name.is_empty() { "Fingerprint reader".to_string() } else { name }).detail(detail).tone(if !install_error.is_empty() || !fix_error.is_empty() { 2 } else { 0 });
            if slow && missing.is_empty() {
                let id = cx.on(|cx, _| cx.ask("system", "fpFix", Value::Null));
                r = r.buttons(vec![btn(if fixing { "Setting up…" } else { "Set Up Reader" }).icon("fingerprint").primary().on(fixing)], id);
            } else if !missing.is_empty() {
                let installing = fp["installing"].as_bool().unwrap_or(false);
                let id = cx.on(|cx, _| cx.ask("system", "fpInstall", Value::Null));
                r = r.buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").primary().on(installing).off(updates_running)], id);
            }
            rows.push(r);
        }

        // 2. The fingers
        let enrolled = strings(&fp["enrolled"]);
        if ready {
            for f in &enrolled {
                let asking = confirming == *f;
                let name = finger_name(sys, f);
                let fc = f.clone();
                let id = cx.on(move |cx, a| {
                    // (asking: [Keep, Remove …]; else [Remove])
                    if asking && a.i == 0 {
                        cx.local_set("bio.confirming", "");
                    } else if !asking {
                        cx.local_set("bio.confirming", fc.clone());
                    } else {
                        cx.local_set("bio.confirming", "");
                        cx.ask("system", "fpRemove", json!([fc.clone()]));
                    }
                });
                let mut b = Vec::new();
                if asking {
                    b.push(btn("Keep"));
                }
                b.push(btn(if asking { format!("Remove {name}") } else { "Remove".into() }).danger());
                rows.push(Row::new(name).icon("fingerprint").detail(if asking { "It's deleted from the reader: you'd add it again to use it." } else { "" }).buttons(b, id));
            }
            // Adding one: the free fingers, the one picked
            let free: Vec<(String, String)> = fingers(sys).into_iter().filter(|f| !enrolled.contains(&f.0)).collect();
            let pick = cx.local_s("bio.pick");
            let chosen = if free.iter().any(|f| f.0 == pick) { pick } else { free.first().map(|f| f.0.clone()).unwrap_or_default() };
            let busy = fp["enrolling"].as_bool().unwrap_or(false) || s(&fp["enrollStage"]) == "waiting";
            let keys: Vec<String> = free.iter().map(|f| f.0.clone()).collect();
            let names: Vec<String> = free.iter().map(|f| f.1.clone()).collect();
            let index = keys.iter().position(|k| *k == chosen).map_or(-1, |i| i as i32);
            let pick_id = cx.on(move |cx, a| {
                if let Some(k) = keys.get(a.i.max(0) as usize) {
                    cx.local_set("bio.pick", k.clone());
                }
            });
            let ch = chosen.clone();
            let add = cx.on(move |cx, _| {
                cx.local_set("fp.sheet", ch.clone());
                cx.ask("system", "fpEnroll", json!([ch.clone()]));
            });
            // (a Dropdown and Add side by side: the window's menu opens for
            // the dropdown, its items given here as a Choice's would be)
            let items = names.iter().enumerate().map(|(i, n)| MenuItem::new(n.clone()).checked(i as i32 == index)).collect();
            cx.menu(&pick_id, items);
            let mut r = Row::new(if enrolled.is_empty() { "Add a fingerprint" } else { "Add another" })
                .detail("The reader is read 11 times, a different part of your finger each time")
                .choice(&names, index, pick_id.clone())
                .width(190.0);
            r.control = "custom";
            r.group = "system".into();
            r.sub = "pick-add".into();
            rows.push(r.data(vec![Opt::new("Add").icon("fingerprint").off(busy || chosen.is_empty()).id(add)]));

            // 3. Where a finger works
            let lock = cx.b("lock.fingerprint");
            let t = cx.toggle("lock.fingerprint");
            rows.push(Row::new("Unlock the lock screen").detail("Touch the reader or type your password").switch(lock, t));
            let pam_busy = s(&fp["pamBusy"]);
            let gdm = fp["gdm"].as_bool().unwrap_or(false);
            let places = [
                ("polkit", "Password prompts", "When an app or Tessera asks for an administrator's password".to_string()),
                ("sudo", "sudo in terminals", "sudo asks for a finger first".to_string()),
                (
                    "login",
                    "Login screen",
                    if gdm {
                        "GDM asks for a finger by itself. For greetd, ly, SDDM, LightDM or LXDM: your password is still needed once after a restart (it unlocks your saved passwords)".to_string()
                    } else {
                        "Your password is still needed once after a restart: it unlocks your saved passwords".to_string()
                    },
                ),
            ];
            for (key, label, detail) in places {
                let on = fp["pam"][key].as_bool().unwrap_or(false);
                let id = cx.on(move |cx, _| cx.ask("system", "fpSetPam", json!([key, !on])));
                rows.push(Row::new(label).detail(if pam_busy == key { "Changing…".to_string() } else { detail }).switch(on, id).enabled(pam_busy.is_empty()));
            }
        }
        let g = p.group("Fingerprint");
        g.footer(footer);
        for r in rows {
            g.row(r);
        }
    }

    // ── Face: only with an infrared camera; the lock screen only ──
    let face = &sys["face"];
    if face["hasCamera"].as_bool().unwrap_or(false) {
        let installed = face["installed"].as_bool().unwrap_or(false);
        let set_up = face["setUp"].as_bool().unwrap_or(false);
        let faces = face["faces"].as_i64().unwrap_or(0);
        let busy = s(&face["busy"]);
        let error = s(&face["error"]);
        let camera = s(&face["camera"]);
        let footer = if !error.is_empty() {
            error
        } else if !installed {
            format!("Look at the screen to unlock it, with your laptop's infrared camera ({camera}). It's less secure than a password or a fingerprint: someone who looks very like you might get in. Your password always works too.")
        } else {
            "Look at the screen when it's locked. Face unlock is for the lock screen only; password prompts and sudo still ask for your password or finger. Each change asks for your password.".into()
        };
        let mut rows = Vec::new();
        if !installed {
            let ie = s(&face["installError"]);
            let installing = face["installing"].as_bool().unwrap_or(false);
            let id = cx.on(|cx, _| cx.ask("system", "faceInstall", Value::Null));
            rows.push(
                Row::new("Face unlock")
                    .detail(if ie.is_empty() { "Needs Howdy (howdy-next, from the AUR: Tessera sets up an AUR helper first if there's none). It asks for your password once.".to_string() } else { ie.clone() })
                    .tone(if ie.is_empty() { 0 } else { 2 })
                    .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing).off(updates_running)], id),
            );
        }
        if installed && !set_up {
            let id = cx.on(|cx, _| cx.ask("system", "faceSetUp", Value::Null));
            let cam = if camera.is_empty() { "the IR camera".to_string() } else { camera.clone() };
            rows.push(
                Row::new("Set up the camera")
                    .detail(format!("Uses {cam} and downloads Howdy's face models"))
                    .buttons(vec![btn(if busy == "setup" { "Setting up…" } else { "Set Up" }).primary().on(busy == "setup").off(!busy.is_empty() && busy != "setup")], id),
            );
        }
        if set_up {
            let asking = confirming == "faces";
            let label = if busy == "add" {
                "Look at the camera…".to_string()
            } else if faces == 0 {
                "Add your face".into()
            } else if faces == 1 {
                "1 face added".into()
            } else {
                format!("{faces} faces added")
            };
            let detail = if busy == "add" {
                "Hold still for a moment, in normal light"
            } else if faces == 0 {
                "Look at the camera for a few seconds"
            } else {
                "Add another (with glasses, say) for better recognition"
            };
            // [Keep]? [Remove …]? [Add …]
            let mut b = Vec::new();
            let mut acts: Vec<&'static str> = Vec::new();
            if asking {
                b.push(btn("Keep"));
                acts.push("keep");
            }
            if faces > 0 {
                let t = if busy == "clear" {
                    "Removing…".to_string()
                } else if asking {
                    if faces == 1 { "Remove your face".to_string() } else { format!("Remove all {faces} faces") }
                } else {
                    "Remove All".into()
                };
                b.push(btn(t).danger().off(!busy.is_empty()));
                acts.push("remove");
            }
            let add = btn(if faces == 0 { "Add Face" } else { "Add Another" }).icon("smiley").on(busy == "add").off(!busy.is_empty() && busy != "add");
            b.push(if faces == 0 { add.primary() } else { add });
            acts.push("add");
            let id = cx.on(move |cx, a| match acts.get(a.i.max(0) as usize).copied() {
                Some("keep") => cx.local_set("bio.confirming", ""),
                Some("remove") => {
                    if !asking {
                        cx.local_set("bio.confirming", "faces");
                    } else {
                        cx.local_set("bio.confirming", "");
                        cx.ask("system", "faceClear", Value::Null);
                    }
                }
                Some("add") => cx.ask("system", "faceAdd", Value::Null),
                _ => {}
            });
            rows.push(Row::new(label).detail(detail).buttons(b, id));
            if faces > 0 {
                let on = cx.b("lock.face");
                let t = cx.toggle("lock.face");
                rows.push(Row::new("Unlock the lock screen").detail("Look at the screen or type your password").switch(on, t));
            }
        }
        let g = p.group("Face");
        g.footer(footer);
        for r in rows {
            g.row(r);
        }
    }
}

/// Privacy & Security (security.qml)
pub fn security(cx: &mut Ctx, p: &mut Page) {
    // The add-a-finger sheet takes the page while it's open
    let finger = cx.local_s("fp.sheet");
    if !finger.is_empty() {
        sheet(cx, p, &finger);
        return;
    }
    let feed = cx.feed;
    let sys = &feed["system"];

    // Fingerprint and Face (both shown only with a reader or an IR camera)
    p.part(cx, "", biometrics);

    let island = cx.b("island.enabled");
    let ind = cx.s("privacy.screenShareIndicator");
    let values = ["island", "dot", "both"];
    let pick = cx.pick("privacy.screenShareIndicator", &values);
    p.group("Privacy").row(
        Row::new("Screen sharing shows")
            .detail(if !island {
                "The island is off, so a share shows as the purple dot."
            } else if ind == "island" {
                "In the island while an app shares your screen, with a Stop button; no dot meanwhile."
            } else if ind == "dot" {
                "A purple dot with the other privacy dots; click it to see what's shared or stop it."
            } else {
                "In the island and as a purple dot."
            })
            .segmented(&["Island", "Dot", "Both"], values.iter().position(|v| *v == ind).unwrap_or(0) as i32, pick)
            .width(240.0),
    );

    let check = &sys["check"];
    let passwordless = check["passwordless"].as_bool().unwrap_or(false);
    let qs = &sys["quickShare"];
    let trust_error = s(&qs["trustError"]);
    let trusted = qs["trusted"].as_bool().unwrap_or(false);
    let trust_busy = qs["trustBusy"].as_bool().unwrap_or(false);
    let trust = cx.on(move |cx, _| cx.ask("system", "quickShareTrust", json!([!trusted])));
    let hotspot = cx.get("sharing.quickShareHotspot") != Value::Bool(false);

    let ports = &sys["ports"];
    let perr = s(&ports["error"]);
    let fw = &check["firewall"];
    let kind = s(&fw["kind"]);
    let open = fw["open"].as_array().map_or(0, Vec::len);
    let fw_detail = firewall_detail(&perr, &kind, open, &s(&fw["why"]), fw["asked"].as_bool().unwrap_or(false));
    let failed64 = ports["failedWith"].as_i64() == Some(64);
    let pbusy = ports["busy"].as_bool().unwrap_or(false);
    let mut fw_row = Row::new("Firewall").detail(fw_detail).tone(if perr.is_empty() { 0 } else { 2 });
    if !perr.is_empty() {
        let id = cx.on(move |cx, _| {
            if failed64 {
                cx.go("update", "");
            } else {
                cx.ask("system", "portsRetry", Value::Null);
            }
        });
        fw_row = fw_row.buttons(vec![btn(if failed64 { "Open Software Update" } else { "Try again" }).icon(if failed64 { "arrow-circle-up" } else { "shield-check" }).on(pbusy)], id);
    }

    p.group("Without your password")
        .footer("Only for an administrator at this computer, in the session on screen. Everything else Tessera does as an administrator asks for your password.")
        .row(Row::new("Tessera's helper").detail(if passwordless {
            "Installed: the login screen's wallpaper and look, and the fixes below, don't ask for your password"
        } else {
            "Not installed yet: the login screen's wallpaper and the fixes below ask for your password. The next update installs it."
        }))
        .row(
            Row::new("Fast transfers without asking")
                .visible(hotspot)
                .detail(if !trust_error.is_empty() {
                    trust_error.clone()
                } else if trusted {
                    "Quick Share's direct Wi-Fi link is set up without your password. Any app running as you could set one up too; turn this off to be asked again.".into()
                } else {
                    "Off: your password is asked each time Quick Share sets up a direct Wi-Fi link. Changing this asks for it once.".into()
                })
                .tone(if trust_error.is_empty() { 0 } else { 2 })
                .switch(trusted, trust)
                .enabled(!trust_busy),
        )
        .row(fw_row);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_firewall_says_where_it_stands() {
        assert_eq!(firewall_detail("", "", 0, "", false), "No firewall stands in sharing's way.");
        assert!(firewall_detail("", "ufw", 2, "", false).contains("while you're sharing,"));
        assert!(firewall_detail("", "ufw", 0, "", true).ends_with("Closed now."));
        assert_eq!(firewall_detail("Broken", "ufw", 0, "", true), "Broken");
    }

    #[test]
    fn the_sheet_names_the_next_touch() {
        let (t, x) = sheet_texts(false, false, false, "right index", "", "", "", 0);
        assert_eq!(t, "Add your right index");
        assert!(x.starts_with("Rest the middle"));
        let (t, _) = sheet_texts(false, false, true, "thumb", "", "", "", 6);
        assert_eq!(t, "Now the edges");
        let (_, x) = sheet_texts(false, false, false, "thumb", "Lift it", "", "", 2);
        assert_eq!(x, "Lift it");
        let (t, _) = sheet_texts(true, false, false, "thumb", "", "done", "", -1);
        assert_eq!(t, "Your thumb is added");
    }

    #[test]
    fn fingers_keep_their_order() {
        let f = serde_json::json!({"fingerprint": {"fingerOrder": ["a", "b"], "fingerNames": {"a": "Thumb", "b": "Index"}}});
        assert_eq!(fingers(&f), vec![("a".to_string(), "Thumb".to_string()), ("b".to_string(), "Index".to_string())]);
        assert_eq!(finger_name(&f, "b"), "Index");
        assert_eq!(finger_name(&f, "z"), "z");
    }
}
