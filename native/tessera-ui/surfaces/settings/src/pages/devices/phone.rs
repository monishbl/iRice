//! Phone & Sharing (pages/phone.qml, with sharing.qml as its part): an
//! Android phone on this computer (services/Phone.qml: its tools, setting it
//! up, its screen here, its camera as a webcam), then sending and receiving
//! files nearby (LocalSend, Quick Share, the browser's QR link, file
//! managers' menus). The phone's and sharing's state is the shell's
//! (feed.phone, feed.sharing); every action goes there.

use super::super::super::model::{BKind, Ctx, Opt, Page, Row};
use super::{ask, feed, place, s};
use serde_json::{json, Value};

/// "Android 14 · 80% charging · Wireless" (phone.qml's detail)
pub fn phone_detail(ph: &Value) -> String {
    let android = format!("Android {}", s(&ph["android"]));
    let battery = ph["battery"].as_i64().filter(|b| *b >= 0).map_or(String::new(), |b| format!("{b}%{}", if ph["charging"].as_bool() == Some(true) { " charging" } else { "" }));
    let how = if ph["wireless"].as_bool() == Some(true) { "Wireless" } else { "USB" }.to_string();
    [android, battery, how].into_iter().filter(|x| !x.is_empty() && x != "Android ").collect::<Vec<_>>().join(" · ")
}

/// The pairing row's label and detail (phone.qml), by Phone.pairStage
pub fn pair_text(stage: &str, error: &str) -> (&'static str, String) {
    let label = match stage {
        "starting" => "Getting a code…",
        "waiting" => "Scan this code on your phone",
        "pairing" => "Pairing…",
        "connecting" => "Connecting…",
        "done" => "Paired",
        _ => "Pair with a QR code",
    };
    let detail = match stage {
        "waiting" => "On the phone: Developer options → Wireless debugging → Pair device with QR code".to_string(),
        "error" => {
            if error.is_empty() {
                "Pairing didn't work. Check the phone is on the same Wi-Fi, then try again.".into()
            } else {
                error.to_string()
            }
        }
        "done" => "Your phone is connected over Wi-Fi.".into(),
        _ => "Your phone connects over Wi-Fi from then on".into(),
    };
    (label, detail)
}

/// A button with an icon (a row's own icon is drawn before its label, so
/// an icon on its button goes this way)
fn icon_button(text: &str, icon: &str, primary: bool, busy: bool, enabled: bool) -> Vec<Opt> {
    let mut o = Opt::new(text).icon(icon).on(busy).off(!enabled);
    if primary {
        o = o.primary();
    }
    vec![o]
}

pub fn phone(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let ph = &f["phone"];
    let b = |k: &str| ph[k].as_bool().unwrap_or(false);
    let phone = ph["phone"].clone();
    let has_phone = phone.is_object();
    let needs_tools = !b("hasAdb") || !b("hasScrcpy") || !b("qrencode");
    let stage = s(&ph["pairStage"]);
    let pairing = ["starting", "waiting", "pairing", "connecting"].contains(&stage.as_str());
    let updates = f["updatesRunning"].as_bool().unwrap_or(false);
    let install_error = s(&ph["installError"]);
    let installing = b("installing");

    // What it needs
    if needs_tools {
        let id = cx.on(|cx, _| ask(cx, "phoneInstall", json!([false])));
        p.group("")
            .above(24.0)
            .footer("scrcpy shows the phone's screen in a window and lets you use it with your mouse and keyboard. Both are Google's and open source tools.")
            .row(Row::new("Android tools and scrcpy")
                .detail(if install_error.is_empty() { "To see and use your phone here. It asks for your password once.".to_string() } else { install_error.clone() })
                .tone(if install_error.is_empty() { 0 } else { 2 })
                .buttons(icon_button(if installing { "Installing…" } else { "Install" }, "download-simple", true, installing, !updates), id));
    }

    // The phone, or how to connect one
    let serial = s(&phone["serial"]);
    let mirroring = b("mirroring");
    let sr = serial.clone();
    let show = cx.on(move |cx, _| ask(cx, "phoneMirror", json!([sr])));
    let enabled = cx.b("phone.enabled");
    let waiting = b("waiting");
    p.group("Phone")
        .above(24.0)
        .footer(if has_phone {
            "Drop files on the phone's window to copy them to its Downloads folder. Right-click the phone in the Dock to come back here."
        } else {
            "Once, on the phone: Settings → About phone → tap Build number 7 times; then Settings → System → Developer options → turn on USB debugging. Connect it with a cable and tap Allow."
        })
        .row(Row::new(s(&phone["model"]))
            .visible(has_phone)
            .detail(phone_detail(&phone))
            .buttons(icon_button(if mirroring { "Showing" } else { "Show Screen" }, "device-mobile", !mirroring && !needs_tools, false, b("hasScrcpy")), show))
        .row(Row::new("Allow this computer on your phone")
            .visible(!has_phone && waiting)
            .detail("Unlock the phone and tap Allow on the USB debugging prompt (tick Always allow from this computer)."))
        .row(Row::new("No phone connected")
            .visible(!has_phone && !waiting)
            .detail(if enabled { "Connect it with a USB cable, or pair it without one below." } else { "Looking for phones is off." }));

    // Without a cable
    let needs = &f["needs"];
    let fixing = needs["installing"].as_bool().unwrap_or(false) || s(&f["healthFixing"]) == "avahi";
    let avahi = cx.on(|cx, _| ask(cx, "avahiOn", Value::Null));
    let pair = cx.on(move |cx, _| ask(cx, if pairing { "phoneCancelPair" } else { "phonePair" }, Value::Null));
    let (label, detail) = pair_text(&stage, &s(&ph["pairError"]));
    let qr = s(&ph["pairQr"]);
    let mut code = Row::custom("devices", "qr").visible(stage == "waiting" && !qr.is_empty());
    code.step = place(false, true);
    if stage == "waiting" && !qr.is_empty() {
        code.image = cx.picture(&qr, 312, 312);
    }
    p.group("Without a cable")
        .above(24.0)
        .footer("Android 11 or later, on the same Wi-Fi as this computer. Many phones turn wireless debugging off after a restart or a change of Wi-Fi: pair again then, or use the cable.")
        .row(Row::new("Network discovery is off")
            .visible(needs["checked"].as_bool().unwrap_or(false) && !needs["avahi"].as_bool().unwrap_or(true))
            .detail("Pairing finds the phone with avahi, which isn't running. It asks for your password once.")
            .button(if fixing { "Turning on…" } else { "Turn On" }, avahi)
            .busy(fixing))
        .row(Row::new(label).detail(detail).tone(if stage == "error" { 2 } else { 0 }).button(if pairing { "Cancel" } else { "Pair" }, pair).enabled(b("hasAdb")))
        .row(code);

    // Its camera
    let cam_ready = b("camReady");
    let cam_module = b("camModule");
    let cam_error = s(&ph["camError"]);
    let kernel = s(&ph["kernelPackage"]);
    let webcam = b("webcam");
    let front = cx.s("phone.camera") == "front";
    let mut rows = Vec::new();
    if !cam_ready {
        let detail = if !install_error.is_empty() && !needs_tools {
            install_error.clone()
        } else if !cam_module {
            format!("Needs a virtual camera (v4l2loopback{}). Install it, then set it up. Each asks for your password once.", if kernel.is_empty() { String::new() } else { format!(", built for your {kernel} kernel") })
        } else {
            "Set up a virtual camera for it. It asks for your password once.".into()
        };
        let r = Row::new("Phone camera").detail(detail);
        rows.push(if !cam_module {
            let id = cx.on(|cx, _| ask(cx, "phoneInstall", json!([true])));
            r.button(if installing { "Installing…" } else { "Install" }, id).busy(installing).enabled(!updates)
        } else {
            let setting = b("camSetting");
            let id = cx.on(|cx, _| ask(cx, "phoneSetUpCam", Value::Null));
            r.button(if setting { "Setting up…" } else { "Set Up" }, id).busy(setting)
        });
    } else {
        let t = cx.on(move |cx, _| ask(cx, "phoneWebcam", json!([!webcam])));
        rows.push(Row::new("Use as webcam")
            .detail(if !has_phone { "Connect your phone first".to_string() } else if webcam { format!("On: the phone's {} camera", if front { "front" } else { "back" }) } else { "Off".into() })
            .switch(webcam, t)
            .enabled(has_phone && b("hasScrcpy")));
        let pick = cx.on(|cx, a| {
            cx.set("phone.camera", if a.i == 1 { "front" } else { "back" });
            ask(cx, "phoneCameraChanged", Value::Null);
        });
        rows.push(Row::new("Camera").segmented(&["Back", "Front"], if front { 1 } else { 0 }, pick).width(200.0));
    }
    let g = p.group("Webcam");
    g.above(24.0).footer(if cam_error.is_empty() { "Your phone's camera as a webcam for calls and meetings: choose “Phone camera” in the app. Needs Android 12 or later.".to_string() } else { cam_error });
    for r in rows {
        g.row(r);
    }

    // ── More options ──
    let more_at = p.blocks.len();
    p.more(cx);
    {
        let (e, d, so) = (cx.b("phone.enabled"), cx.b("phone.dock"), cx.b("phone.screenOff"));
        let (te, td, ts) = (cx.toggle("phone.enabled"), cx.toggle("phone.dock"), cx.toggle("phone.screenOff"));
        p.group("Options")
            .above(24.0)
            .row(Row::new("Look for phones").detail("Off: phones aren't looked for, and the rest of this page waits").switch(e, te))
            .row(Row::new("Show the phone in the Dock").detail("While it's connected: click it to see its screen").switch(d, td))
            .row(Row::new("Keep the phone's screen off").detail("While it's shown here (saves its battery; it comes back on when you disconnect)").switch(so, ts));
    }
    p.end_more();
    if let Some(m) = p.blocks.get_mut(more_at) {
        m.above = Some(24.0);
    }

    let part_at = p.blocks.len();
    p.part(cx, "Sharing", sharing);
    // (phone.qml's own spacing is 24: the part sits 24 under the page)
    if let Some(h) = p.blocks.get_mut(part_at).filter(|b| b.kind == BKind::Heading) {
        h.above = Some(24.0);
    }
}

/// Sharing (sharing.qml)
pub fn sharing(cx: &mut Ctx, p: &mut Page) {
    let f = feed(cx);
    let sh = &f["sharing"];
    let b = |k: &str| sh[k].as_bool().unwrap_or(false);

    // Nearby sharing (LocalSend)
    let local = b("localSend");
    let lerr = s(&sh["localSendError"]);
    let send = cx.on(|cx, _| ask(cx, "sendFiles", Value::Null));
    let rv = ["off", "10min", "everyone"];
    let receiving = cx.s("sharing.receiving");
    let rp = cx.pick("sharing.receiving", &rv);
    let ports = s(&sh["portsError"]);
    let show = cx.on(|cx, _| cx.go("security", "Firewall"));
    let name = cx.s("sharing.name");
    let nf = cx.on(|cx, a| {
        if a.i == 1 {
            let t = a.s.trim().to_string();
            if t != cx.s("sharing.name") {
                cx.set("sharing.name", t);
            }
        }
    });
    p.group("Nearby sharing")
        .footer(if !local {
            "Sharing needs Python 3 and openssl (for encrypted transfers).".to_string()
        } else {
            format!(
                "Works with LocalSend on phones and computers on the same network. Files you accept go to Downloads. With a firewall on, Tessera lets nearby devices through it only while you're visible or sharing, and closes it after.{}",
                if lerr.is_empty() { String::new() } else { format!(" {lerr}") }
            )
        })
        .row(Row::new("Send files")
            .detail("Choose files, then a nearby device: one running LocalSend, or an Android phone with Quick Share. Receiving can stay off.")
            .buttons(icon_button("Send files…", "paper-plane-tilt", false, false, local && !b("pickerBusy")), send))
        .row(Row::new("Receive files from")
            .detail(if receiving == "off" { "You can still send; nobody can see you" } else { "You're asked before anything is saved" })
            .segmented(&["No one", "Everyone for 10 min", "Everyone"], rv.iter().position(|v| *v == receiving).unwrap_or(0) as i32, rp)
            .width(320.0))
        .row(Row::new("Firewall").visible(!ports.is_empty()).detail(ports.clone()).button("Show", show))
        .row(Row::new("Your name").detail("How you appear on other devices").field(name, format!("{}'s computer", s(&sh["user"])), nf).width(220.0));

    // Android phones (Quick Share)
    let qs = b("quickShare");
    let qerr = s(&sh["quickShareError"]);
    let qv = cx.s("sharing.quickShare");
    let qp = cx.pick("sharing.quickShare", &rv);
    let bt_available = b("btAvailable");
    let bt_on = b("btOn");
    let bt = cx.on(|cx, _| ask(cx, "quickShareBluetooth", Value::Null));
    let fixing = s(&sh["fixing"]);
    let fix = cx.on(|cx, _| ask(cx, "sharingFix", json!(["pairing"])));
    let hotspot = cx.get("sharing.quickShareHotspot") != Value::Bool(false);
    let ht = cx.on(move |cx, _| cx.set("sharing.quickShareHotspot", !hotspot));
    let folder = cx.s("sharing.quickShareFolder");
    let fp = cx.path_setting("sharing.quickShareFolder", true, "");
    let link = cx.on(|cx, _| ask(cx, "receiveLink", Value::Null));
    let ask_note = if b("passwordless") { "" } else { " asks for your password and" };
    let fix_error = s(&sh["fixError"]);
    p.group("Android phones (Quick Share)")
        .footer(format!(
            "{}{}",
            if !qs {
                "Quick Share needs Python 3 with python-cryptography and python-protobuf.".to_string()
            } else {
                format!("Phones nearby find this computer in their Quick Share sheet as “{}” (over the same Wi-Fi, or Bluetooth), and you're asked before anything is saved, with the PIN the phone shows. To send, use Share on a file and pick the phone (its Quick Share open, visible to Everyone).", s(&sh["quickShareName"]))
            },
            if qerr.is_empty() { String::new() } else { format!(" {qerr}") }
        ))
        .row(Row::new("Visible to phones")
            .detail(if qv == "off" { "You can still send; phones can't see this computer" } else { "Phones nearby can offer you files, links and Wi-Fi networks" })
            .segmented(&["No one", "Everyone for 10 min", "Everyone"], rv.iter().position(|v| *v == qv).unwrap_or(0) as i32, qp)
            .width(320.0))
        .row({
            let r = Row::new("Bluetooth").detail(if !bt_available {
                "No Bluetooth adapter: phones find this computer only on the same network"
            } else if bt_on {
                "On: phones find this computer without a shared Wi-Fi too, and it finds them"
            } else {
                "Turn on Bluetooth so phones can find this computer without Wi-Fi"
            });
            if bt_available && !bt_on {
                r.buttons(icon_button("Turn on Bluetooth", "bluetooth", false, false, true), bt)
            } else {
                r
            }
        })
        .row(Row::new("Phones ask to pair")
            .visible(b("pairingRisk"))
            .detail(format!("Some of Bluetooth's experimental features (Ranging and LE Audio controls) make phones ask to pair when they send with Quick Share; phones never need to. Turning them off{ask_note} restarts Bluetooth (headphones reconnect, and still show their battery).{}", if fix_error.is_empty() { String::new() } else { format!(" {fix_error}") }))
            .buttons(icon_button("Stop pairing prompts", "bluetooth", false, fixing == "pairing", fixing.is_empty()), fix))
        .row(Row::new("Fast transfers without a shared Wi-Fi")
            .detail(if hotspot {
                "A direct Wi-Fi link with the phone, next to your own Wi-Fi where this computer's Wi-Fi allows it (else your Wi-Fi pauses during the transfer, then comes back)"
            } else {
                "Off: they go over Bluetooth, which is slower; the Wi-Fi isn't touched"
            })
            .switch(hotspot, ht))
        .row(Row::new("Save files to").path(folder, "~/Downloads", fp).busy(b("pickerBusy")))
        .row(Row::new("iPhone and other phones")
            .detail("Show a QR code their camera opens: they send photos and files from the browser, on the same Wi-Fi. To send to them, use Share on a file → Share with a QR code.")
            .buttons(icon_button("Receive from iPhone", "qr-code", false, false, true), link));

    // Share from anywhere
    let managers: Vec<Value> = sh["managers"].as_array().cloned().unwrap_or_default();
    let shown: Vec<Value> = managers.iter().filter(|m| m["found"].as_bool() == Some(true) || m["installed"].as_bool() == Some(true)).cloned().collect();
    let fm_error = s(&sh["fmError"]);
    let busy = b("fmBusy");
    let mut rows = Vec::new();
    for m in &shown {
        let installed = m["installed"].as_bool().unwrap_or(false);
        let id_ = s(&m["id"]);
        let note = s(&m["note"]);
        let id = cx.on(move |cx, _| ask(cx, "fileManager", json!([if installed { "remove" } else { "install" }, id_])));
        rows.push(Row::new(s(&m["name"]))
            .detail(if installed { format!("“Send to a nearby device…” is in its right-click menu{}", if note.is_empty() { String::new() } else { format!(". {note}") }) } else { "Not added".into() })
            .buttons(icon_button(if installed { "Remove menu" } else { "Add menu" }, if installed { "minus" } else { "plus" }, false, busy, true), id));
    }
    let sel = cx.on(|cx, _| ask(cx, "shareSelection", Value::Null));
    rows.push(Row::new("Share the selection")
        .detail("Sends the selected text (or what you copied) to a nearby device. The shortcut is Super+Ctrl+S; change it in Keyboard Shortcuts.")
        .buttons(icon_button("Share selection", "share-network", false, false, true), sel));
    let g = p.group("Share from anywhere");
    g.footer(if !fm_error.is_empty() {
        fm_error
    } else if managers.iter().any(|m| m["found"].as_bool() == Some(true)) {
        "Right-click files in these file managers to send them to a nearby device. The menus can be removed any time; nothing else in them changes.".into()
    } else {
        "No supported file manager found (Files, Dolphin, Thunar or Nemo). Tessera's own desktop and Downloads stack have Share… either way.".into()
    });
    for r in rows {
        g.row(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phone_is_described_as_phone_qml_does() {
        assert_eq!(phone_detail(&json!({"android": "14", "battery": 80, "charging": true, "wireless": true})), "Android 14 · 80% charging · Wireless");
        assert_eq!(phone_detail(&json!({"battery": -1})), "USB");
        assert_eq!(pair_text("waiting", "").0, "Scan this code on your phone");
        assert_eq!(pair_text("error", "").1, "Pairing didn't work. Check the phone is on the same Wi-Fi, then try again.");
        assert_eq!(pair_text("", "").0, "Pair with a QR code");
    }
}
