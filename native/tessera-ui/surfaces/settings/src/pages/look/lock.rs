//! Lock & Login Screen (pages/lock.qml), with Material's clock faces
//! (ClockFaces.qml) and the boot animations (BootAnimations.qml), both
//! drawn in settings-look.slint.

use super::super::super::model::{Ctx, Opt, Page, Row};
use super::{feed, idx, place, wallpaper_path};
use serde_json::{json, Value};

/// Material's lock clock faces: (id, name)
pub const FACES: [(&str, &str); 6] = [("dynamic", "Dynamic"), ("stacked", "Stacked"), ("wide", "Wide"), ("overlap", "Overlap"), ("analog", "Analog"), ("weather", "Weather")];

/// A Segmented for a lock setting (lock.qml's Choice)
fn choice(cx: &mut Ctx, label: &str, key: &str, values: &[&str], options: &[&str], width: f32) -> Row {
    let path = format!("lock.{key}");
    let cur = cx.s(&path);
    let pick = cx.pick(&path, values);
    Row::new(label).segmented(options, idx(values, &cur), pick).width(width)
}

/// The boot animation's footer (BootAnimations' state, as lock.qml says it)
pub fn boot_footer(b: &Value) -> String {
    let s = |k: &str| b[k].as_str().unwrap_or("").to_string();
    let t = |k: &str| b[k].as_bool().unwrap_or(false);
    if !s("error").is_empty() {
        return s("error");
    }
    if t("busy") {
        return "Rebuilding the boot image. This takes about a minute; you can keep working.".into();
    }
    if !s("blocked").is_empty() {
        return s("blocked");
    }
    if t("done") {
        return "Done. It shows the next time your computer starts.".into();
    }
    if t("checked") && !b["splash"].as_bool().unwrap_or(true) {
        return "Your boot settings hide the animation: add splash to GRUB_CMDLINE_LINUX_DEFAULT in /etc/default/grub, then run sudo grub-mkconfig -o /boot/grub/grub.cfg.".into();
    }
    let chosen = s("chosen");
    let design = b["designs"].as_array().and_then(|d| d.iter().find(|x| x["id"].as_str() == Some(chosen.as_str()))).and_then(|d| d["description"].as_str()).map(str::to_string);
    let what = match design {
        Some(d) => d,
        None if chosen == "none" => "No picture: the boot messages, as text.".into(),
        None => {
            let o = s("original");
            format!("Your system's own{}", if o.is_empty() { ".".to_string() } else { format!(" ({o}).") })
        }
    };
    format!("{what} Changing it asks for your password.")
}

pub fn lock(cx: &mut Ctx, p: &mut Page) {
    let material = cx.material;
    let h24 = cx.b("appearance.clock24");

    // Material: the clock, Android 17's styles from a carousel, its size and thickness
    if material {
        let face = cx.s("lock.clockFace");
        let faces: Vec<Opt> = FACES.iter().map(|(id, name)| Opt::new(*name).id(*id).on(face == *id)).collect();
        let fp = cx.on(|cx, a| {
            if let Some((id, _)) = FACES.get(a.i.max(0) as usize) {
                cx.set("lock.clockFace", *id);
            }
        });
        let wall = wallpaper_path(cx);
        let mut r = Row::custom("look", "clock-faces").id(fp).data(faces).num(place(true, false));
        // the time the faces show: hours, minutes, and AM/PM (the date with 24-hour time)
        r.text = crate::util::now(if h24 { "%H" } else { "%-I" });
        r.low = format!("{:0>2}", r.text);
        r.value = crate::util::now("%M");
        r.index = FACES.iter().position(|f| f.0 == face).unwrap_or(0) as i32;
        // (the analog face's hands: minutes since midnight)
        r.to = crate::util::now("%H").parse::<f32>().unwrap_or(0.0) * 60.0 + r.value.parse::<f32>().unwrap_or(0.0);
        r.placeholder = if h24 { crate::util::now("%-d %b") } else { crate::util::now("%p") };
        r.step = cx.f("lock.clockWeight");
        r.from = cx.f("lock.clockWidth");
        r.on = cx.b("lock.clockRound");
        let w = &feed(cx)["weather"];
        if w["ready"].as_bool().unwrap_or(false) {
            r.detail = format!("{}°", w["temp"].as_i64().unwrap_or(0));
            r.icon = w["icon"].as_str().unwrap_or("sun").to_string();
        }
        if let Some(i) = cx.picture(&wall, 352, 376) {
            r = r.image(i);
        }
        let size = choice(cx, "Size", "clockSize", &["large", "small"], &["Large", "Small"], 300.0);
        let weight = cx.f("lock.clockWeight");
        let wn = cx.num("lock.clockWeight", 1.0);
        p.group("Clock").row(r).row(size).row(Row::new("Thickness").slider(weight, 100.0, 1000.0, 20.0, wn).width(220.0));
    }

    let layout = cx.s("lock.layout");
    let notes = cx.s("lock.notifications");
    let footer = if material {
        match notes.as_str() {
            "shelf" => "Under the clock: which apps have notifications, and the newest one. Now Playing sits over your account.",
            "column" => "Beside the clock, like a tablet: Now Playing and the latest notifications.",
            "icons" => "Under the clock, only which apps have notifications. Now Playing sits over your account.",
            _ => "",
        }
    } else if layout == "macos" {
        "The clock up top; your picture, name and the password field at the bottom."
    } else {
        "A large clock with small widgets, Now Playing and your notifications, like iPhone."
    };
    let lr = choice(cx, "Layout", "layout", &["ios", "macos"], &["Widgets", "Classic"], 300.0).detail(if material {
        if layout == "macos" { "The clock and the date, nothing under them" } else { "Weather, your next event and to-dos under the clock; the battery in the corner" }
    } else {
        ""
    });
    let nr = choice(cx, "Notifications", "notifications", &["shelf", "column", "icons"], &["Newest", "Beside", "Icons"], 300.0).detail(if material {
        ""
    } else {
        match notes.as_str() {
            "shelf" => "Under the clock: which apps have notifications, and the newest one. Now Playing sits over the sign-in.",
            "column" => "Beside the clock on a wide screen: Now Playing and the latest notifications.",
            "icons" => "Under the clock, only which apps have notifications",
            _ => "",
        }
    });
    let previews = cx.b("lock.previews");
    let pt = cx.toggle("lock.previews");
    p.group("").footer(footer).row(lr).row(nr).row(Row::new("Show notification previews").detail("Off shows only which app sent each notification").switch(previews, pt));

    let standby = cx.b("lock.standby");
    let sb = cx.toggle("lock.standby");
    p.group("Bedside Mode")
        .footer("While charging and left alone, the screen shows a large clock, your widgets or photos instead of going dark. Swipe or scroll to change; any key brings your desktop back.")
        .row(Row::new("Bedside Mode").switch(standby, sb));

    login_screen(cx, p);

    // Starting up: the boot animation (Plymouth: BootSplash) and the login intro
    if !cx.local_b("look.bootRefreshed") {
        cx.local_set("look.bootRefreshed", true);
        cx.ask("look", "bootRefresh", Value::Null);
    }
    let boot = feed(cx)["boot"].clone();
    let (os_name, os_logo) = super::appearance::os_release();
    let pending = boot["pending"].as_str().unwrap_or("").to_string();
    let busy = !pending.is_empty();
    let chosen = if busy { pending.clone() } else { boot["chosen"].as_str().unwrap_or("system").to_string() };
    let usable = boot["blocked"].as_str().unwrap_or("").is_empty() && !busy;
    let mut choices: Vec<(String, String)> = vec![("system".into(), format!("System ({os_name}'s)"))];
    if let Some(d) = boot["designs"].as_array() {
        choices.extend(d.iter().map(|x| (x["id"].as_str().unwrap_or("").to_string(), x["name"].as_str().unwrap_or("").to_string())));
    }
    choices.push(("none".into(), "None (text)".into()));
    let logo = super::theme_icon(&[os_logo.as_str(), "distributor-logo", "computer"]);
    let thumbs = crate::util::asset("shell/assets/plymouth/thumbnails");
    let mut cards = Vec::new();
    for (id, name) in &choices {
        let kind = if id == "system" { 1 } else if id == "none" { 2 } else { 0 };
        let mut o = Opt::new(name.clone()).id(id.clone()).on(chosen == *id).off(!usable).style(kind).num(if pending == *id { 1.0 } else { 0.0 });
        if kind == 1 {
            if let Some(i) = logo.clone() {
                o = o.image(i);
            }
        } else if kind == 0 {
            if let Some(i) = thumbs.as_ref().and_then(|d| cx.picture(&d.join(format!("{id}.png")).to_string_lossy(), 320, 200)) {
                o = o.image(i);
            }
        }
        cards.push(o);
    }
    let ids: Vec<String> = choices.iter().map(|c| c.0.clone()).collect();
    let set = cx.on(move |cx, a| {
        if let Some(id) = ids.get(a.i.max(0) as usize) {
            cx.ask("look", "bootSet", json!([id]));
        }
    });
    let reduce = cx.b("appearance.reduceMotion");
    let intro = cx.b("lock.loginIntro");
    let it = cx.on(|cx, a| {
        if a.i == 0 {
            cx.ask("look", "playIntro", Value::Null);
        } else {
            let v = cx.b("lock.loginIntro");
            cx.set("lock.loginIntro", !v);
        }
    });
    let mut intro_row = Row::new("Login intro")
        .detail(if reduce {
            "Off while Reduce motion is on (Accessibility)"
        } else if material {
            "As your desktop first appears after you log in, it opens out of the Tessera shape"
        } else {
            "As your desktop first appears after you log in, the wallpaper comes into focus and the menu bar and dock settle in"
        })
        .ctl("look", "intro", it);
    intro_row.on = intro;
    intro_row.busy = reduce; // (Play Intro can't be used)
    p.group("Starting up")
        .footer(boot_footer(&boot))
        .row(Row::new("Boot animation").detail("What shows while your computer starts and shuts down"))
        .row(Row::custom("look", "boot").id(set).data(cards).num(place(false, false)))
        .row(intro_row);

    let lock_now = cx.on(|cx, _| cx.ask("look", "lock", Value::Null));
    p.group("").row(Row::new("Preview").detail("Locks the screen now").button("Lock Screen", lock_now));

    // ── More options ──
    p.more(cx);
    {
        let cards = choice(cx, "Cards and field", "material", &["glass", "frost", "solid", "system"], &["Glass", "Frost", "Solid", "System"], 300.0)
            .visible(!material)
            .detail("What notifications, Now Playing and the password field are made of");
        let text = choice(cx, "Text", "text", &["light", "dark", "auto"], &["Light", "Dark", "Automatic"], 300.0).detail("Automatic picks light or dark text for the wallpaper");
        p.group("").row(cards).row(text);

        let shift = cx.b("lock.standbyShift");
        let st = cx.toggle("lock.standbyShift");
        let wake = cx.b("lock.standbyWakeOnMouse");
        let wt = cx.toggle("lock.standbyWakeOnMouse");
        let op = cx.f("lock.standbyClockOpacity");
        let on = cx.num("lock.standbyClockOpacity", 0.0);
        let weight = choice(cx, "Clock weight", "standbyClockWeight", &["thin", "regular", "bold"], &["Thin", "Regular", "Bold"], 240.0).enabled(standby);
        p.group("Bedside Mode")
            .row(Row::new("Prevent burn-in").detail("Moves the clock every minute and turns the screen dark for a minute every 15 minutes").switch(shift, st).enabled(standby))
            .row(Row::new("Wake with the mouse").detail("Off: only a key or a click brings the lock screen back").switch(wake, wt).enabled(standby))
            .row(Row::new("Clock brightness").detail(format!("{}%", (op * 100.0).round())).slider(op, 0.2, 1.0, 0.05, on).enabled(standby).width(220.0))
            .row(weight);

        let unlock = cx.s("lock.unlockTransition");
        let lockt = cx.s("lock.lockTransition");
        let sbt = cx.s("lock.standbyTransition");
        let u = choice(cx, "Unlocking", "unlockTransition", &["zoom", "fade", "none"], &["Zoom", "Fade", "None"], 240.0).detail(match unlock.as_str() {
            "zoom" => "The lock lifts away, widgets and icons zoom into place and windows fade in, like iPhone",
            "fade" => "The lock fades away and the desktop fades in",
            _ => "",
        });
        let l = choice(cx, "Locking", "lockTransition", &["zoom", "fade", "none"], &["Zoom", "Fade", "None"], 240.0).detail(match lockt.as_str() {
            "zoom" => "The lock screen settles into place as it appears",
            "fade" => "The clock and cards fade in",
            _ => "",
        });
        let s = choice(cx, "Into Bedside Mode", "standbyTransition", &["morph", "fade", "none"], &["Morph", "Fade", "None"], 240.0)
            .detail(if sbt == "morph" { "The clock glides into place and thins" } else { "" })
            .enabled(standby);
        p.group("Transitions").row(u).row(l).row(s);

        // Several displays: which show Bedside Mode, and whether their clocks
        // keep to the same spot
        let screens: Vec<(String, String, i64, i64)> = match feed(cx)["screens"].as_array() {
            Some(a) if !a.is_empty() => a.iter().map(|s| (s["name"].as_str().unwrap_or("").to_string(), s["model"].as_str().unwrap_or("").to_string(), s["width"].as_i64().unwrap_or(0), s["height"].as_i64().unwrap_or(0))).collect(),
            _ => cx.screens.iter().map(|n| (n.clone(), String::new(), 0, 0)).collect(),
        };
        if screens.len() > 1 {
            let same = cx.b("lock.standbySameClock");
            let sc = cx.toggle("lock.standbySameClock");
            let list = cx.list("lock.standbyScreens");
            let all: Vec<String> = screens.iter().map(|s| s.0.clone()).collect();
            let mut rows = vec![Row::new("Same clock position").detail("Every display moves its clock to the same spot").switch(same, sc)];
            for (name, model, w, h) in &screens {
                let on = list.is_empty() || list.contains(name);
                let (n, all) = (name.clone(), all.clone());
                let t = cx.on(move |cx, _| {
                    let list = cx.list("lock.standbyScreens");
                    let on = list.is_empty() || list.contains(&n);
                    let next = standby_screens(&all, &list, &n, on);
                    cx.set("lock.standbyScreens", Value::Array(next.into_iter().map(Value::from).collect()));
                });
                rows.push(Row::new(if model.is_empty() { name.clone() } else { format!("{model}  ·  {name}") }).detail(if *w > 0 { format!("{w} × {h}") } else { String::new() }).switch(on, t));
            }
            let g = p.group("Bedside Mode on displays");
            g.dim(!standby).footer("Displays that are off stay dark while Bedside Mode shows.");
            for r in rows {
                g.row(r);
            }
        }

        if material {
            let face = cx.s("lock.clockFace");
            let width = cx.f("lock.clockWidth");
            let wn = cx.num("lock.clockWidth", 1.0);
            let round = cx.b("lock.clockRound");
            let rp = cx.on(|cx, a| cx.set("lock.clockRound", a.i == 0));
            p.group("Clock")
                .row(Row::new("Width").visible(face != "analog").slider(width, 25.0, 151.0, 1.0, wn).width(220.0))
                .row(Row::new("Digits").visible(face != "analog").segmented(&["Rounded", "Sharp"], if round { 0 } else { 1 }, rp).width(200.0));
        }
    }
    p.end_more();
}

/// Where the login screen stands (LoginScreen's `login` feed: the copy, the
/// login manager on, the root helper's record of a switch, this boot)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginState {
    /// not read yet
    Checking,
    /// another login manager (or the console) signs you in
    Other,
    /// the helper is switching (the password, then a moment)
    Working,
    /// the helper is going back
    Leaving,
    /// switched; Tessera's shows from the next start
    Restart,
    /// Tessera's is the login screen
    Active,
    /// Tessera's didn't start, so the one before came back by itself
    Reverted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoginView {
    pub state: LoginState,
    /// the login manager on now, as people know it ("SDDM")
    pub dm: String,
    /// the one before Tessera's ("" when none is known)
    pub before: String,
    /// why it went back by itself
    pub why: String,
    /// the root helper can switch (an update brings it)
    pub can_switch: bool,
    /// went back by choice, from the next start
    pub off_pending: bool,
    pub error: String,
}

/// A login manager's name as people know it (login_screen.rs's dm_name)
pub fn dm_name(unit: &str) -> String {
    match unit {
        "" => "the console".into(),
        "sddm" => "SDDM".into(),
        "gdm" => "GDM".into(),
        "lightdm" => "LightDM".into(),
        "lxdm" => "LXDM".into(),
        "xdm" => "XDM".into(),
        o => o.into(),
    }
}

pub fn login_view(l: &Value) -> LoginView {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let record = &l["record"];
    let rstate = s(&record["state"]);
    let same_boot = !s(&l["boot"]).is_empty() && s(&record["boot"]) == s(&l["boot"]);
    let dm = s(&l["dm"]);
    let previous = if record["previous"].is_string() { Some(s(&record["previous"])) } else { Some(s(&l["hint"])).filter(|h| !h.is_empty()) };
    let tessera = l["tessera"].as_bool().unwrap_or(false);
    let state = if s(&l["busy"]) == "switch" {
        LoginState::Working
    } else if s(&l["busy"]) == "back" {
        LoginState::Leaving
    } else if !l["checked"].as_bool().unwrap_or(false) {
        if l["installed"].as_bool().unwrap_or(false) { LoginState::Active } else { LoginState::Checking }
    } else if dm == "greetd" && tessera {
        if rstate == "trial" && same_boot { LoginState::Restart } else { LoginState::Active }
    } else if rstate == "reverted" {
        LoginState::Reverted
    } else {
        LoginState::Other
    };
    LoginView {
        state,
        dm: dm_name(&dm),
        before: previous.as_deref().map(dm_name).unwrap_or_default(),
        why: s(&record["reason"]),
        can_switch: l["canSwitch"].as_bool().unwrap_or(false),
        off_pending: rstate == "off" && same_boot,
        error: s(&l["error"]),
    }
}

/// The "Login screen" group: Tessera's style and preview, and switching to
/// it from another login manager (and back)
fn login_screen(cx: &mut Ctx, p: &mut Page) {
    if !cx.local_b("look.login.refreshed") {
        cx.local_set("look.login.refreshed", true);
        cx.ask("look", "loginRefresh", Value::Null);
    }
    let login = feed(cx)["login"].clone();
    let v = login_view(&login);
    let status = login["status"].as_str().unwrap_or("").to_string();
    let switched = matches!(v.state, LoginState::Active | LoginState::Restart);

    // Style: stored only where it shows; before the switch it waits for it
    let values = ["look", "text", "plain"];
    let wanted = cx.local_s("look.login.style");
    let style = if !switched && !wanted.is_empty() { wanted.clone() } else { cx.s("lock.loginStyle") };
    let pick = cx.on(move |cx, a| {
        let Some(v) = values.get(a.i.max(0) as usize) else { return };
        let fb = login_view(&feed(cx)["login"].clone());
        if matches!(fb.state, LoginState::Active | LoginState::Restart | LoginState::Checking) {
            cx.set("lock.loginStyle", *v);
            cx.local_set("look.login.style", "");
        } else {
            cx.local_set("look.login.style", *v);
        }
    });
    let style_detail = if !switched && !wanted.is_empty() && v.state != LoginState::Checking {
        "Shows once you use Tessera's login screen"
    } else {
        match style.as_str() {
            "look" => "Your look, the one you used last: Glass or Material, with your clock and colours",
            "text" => "Like ly: the session, your login and the password in a box of text, keyboard first",
            "plain" => "Black, your picture, your name and the field",
            _ => "",
        }
    };
    let sr = Row::new("Style").segmented(&["Your look", "Text", "Plain"], idx(&values, &style), pick).width(300.0).detail(style_detail);

    let switch = cx.on(|cx, _| {
        let wanted = cx.local_s("look.login.style");
        let style = if wanted.is_empty() { cx.s("lock.loginStyle") } else { wanted };
        cx.set("lock.loginStyle", style.clone());
        cx.local_set("look.login.style", "");
        cx.ask("look", "loginSwitch", json!([style]));
    });
    let back = cx.on(|cx, _| cx.ask("look", "loginBack", Value::Null));
    let restart = cx.on(|cx, _| cx.ask("about", "reboot", Value::Null));
    let preview = cx.on(|cx, _| cx.ask("look", "previewLogin", Value::Null));
    let use_row = |label: String, detail: String| Row::new(label).detail(detail).button("Use Tessera's login screen", switch.clone()).primary().enabled(v.can_switch);
    let back_row = || {
        Row::new(format!("Before: {}", v.before))
            .detail("The login screen this computer had. Going back takes effect at the next restart.")
            .button(format!("Go back to {}", v.before), back.clone())
    };

    let mut rows: Vec<Row> = Vec::new();
    let footer = match v.state {
        LoginState::Checking => String::new(),
        LoginState::Other => {
            let detail = if !v.can_switch {
                "Update Tessera first (Settings → Software Update), then switch here.".to_string()
            } else if v.off_pending {
                format!("{} is back from the next restart. Tessera's login screen shows your look, Text or Plain.", capital(&v.dm))
            } else {
                "Tessera's shows your look, Text or Plain. Switching asks for your password once and takes effect at the next restart.".to_string()
            };
            rows.push(use_row(signs_in_with(&v.dm), detail));
            format!("If Tessera's login screen doesn't start, {} comes back by itself.", v.dm)
        }
        LoginState::Reverted => {
            let why = if v.why.is_empty() { "it didn't start".to_string() } else { v.why.clone() };
            rows.push(use_row("Tessera's login screen didn't start".into(), format!("{} is back because {why}.", capital(&v.dm))));
            "Using it again tries it at the next restart, with the same way back.".into()
        }
        LoginState::Working => {
            rows.push(Row::new("Setting up Tessera's login screen").detail("Enter your password when asked. This takes a moment.").spinner());
            String::new()
        }
        LoginState::Leaving => {
            rows.push(Row::new(format!("Going back to {}", if v.before.is_empty() { "the login screen before".to_string() } else { v.before.clone() })).spinner());
            String::new()
        }
        LoginState::Restart => {
            let before = if v.before.is_empty() { "the one before".to_string() } else { v.before.clone() };
            rows.push(Row::new("Restart to use it").detail(format!("Tessera's login screen replaces {before} when your computer starts again.")).button("Restart now", restart).primary());
            format!("If it doesn't start, {before} comes back by itself and you're told why after you log in.")
        }
        LoginState::Active => {
            if status.is_empty() { "Your colours, light or dark and your clock go to the login screen by themselves.".to_string() } else { status }
        }
    };
    if v.state != LoginState::Working && v.state != LoginState::Leaving {
        rows.push(sr);
        rows.push(Row::new("Preview").detail("Shows it on this screen. The password there is tessera; Esc closes it.").button("Preview login screen", preview));
    }
    if switched && !v.before.is_empty() {
        rows.push(back_row());
    }
    let footer = if v.error.is_empty() { footer } else { v.error.clone() };
    let g = p.group("Login screen");
    g.footer(footer);
    for r in rows {
        g.row(r);
    }
}

/// "This computer signs in with SDDM" (or "on the console")
pub fn signs_in_with(dm: &str) -> String {
    if dm == "the console" { "This computer signs in on the console".into() } else { format!("This computer signs in with {dm}") }
}

/// "the console" → "The console" (a name at the start of a sentence)
fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Bedside Mode's displays after switching one (an empty list: every
/// display, so new ones join in)
pub fn standby_screens(all: &[String], list: &[String], name: &str, on: bool) -> Vec<String> {
    let mut next: Vec<String> = if list.is_empty() { all.to_vec() } else { list.to_vec() };
    if on {
        next.retain(|n| n != name);
    } else {
        next.push(name.to_string());
    }
    if all.iter().all(|n| next.contains(n)) {
        next.clear();
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_for_bedside_mode() {
        let all = vec!["A".to_string(), "B".to_string()];
        assert_eq!(standby_screens(&all, &[], "A", true), vec!["B".to_string()]);
        assert!(standby_screens(&all, &["B".to_string()], "A", false).is_empty());
    }

    #[test]
    fn boot_footer_says_whats_happening() {
        assert_eq!(boot_footer(&json!({"error": "No."})), "No.");
        assert!(boot_footer(&json!({"busy": true})).starts_with("Rebuilding"));
        assert_eq!(boot_footer(&json!({"chosen": "none"})), "No picture: the boot messages, as text. Changing it asks for your password.");
        assert_eq!(boot_footer(&json!({"chosen": "system", "original": "bgrt"})), "Your system's own (bgrt). Changing it asks for your password.");
        assert_eq!(boot_footer(&json!({"chosen": "orbit", "designs": [{"id": "orbit", "description": "Rings."}]})), "Rings. Changing it asks for your password.");
    }

    #[test]
    fn login_screen_states() {
        // SDDM on, nothing switched yet: say so, offer the switch
        let other = json!({"checked": true, "dm": "sddm", "tessera": false, "record": null, "boot": "b1", "canSwitch": true});
        let v = login_view(&other);
        assert_eq!((v.state, v.dm.as_str(), v.can_switch), (LoginState::Other, "SDDM", true));
        assert_eq!(signs_in_with(&v.dm), "This computer signs in with SDDM");
        assert_eq!(signs_in_with(&login_view(&json!({"checked": true, "dm": ""})).dm), "This computer signs in on the console");
        // (a helper from before login-switch: the button waits for an update)
        assert!(!login_view(&json!({"checked": true, "dm": "sddm"})).can_switch);
        // the helper running
        assert_eq!(login_view(&json!({"checked": true, "dm": "sddm", "busy": "switch"})).state, LoginState::Working);
        assert_eq!(login_view(&json!({"checked": true, "dm": "greetd", "tessera": true, "busy": "back"})).state, LoginState::Leaving);
        // switched in this boot: restart to use it; the way back is SDDM
        let trial = json!({"previous": "sddm", "state": "trial", "boot": "b1", "time": 1});
        let v = login_view(&json!({"checked": true, "dm": "greetd", "tessera": true, "record": trial, "boot": "b1"}));
        assert_eq!((v.state, v.before.as_str()), (LoginState::Restart, "SDDM"));
        // after the restart (the check confirms it soon): active
        assert_eq!(login_view(&json!({"checked": true, "dm": "greetd", "tessera": true, "record": trial, "boot": "b2"})).state, LoginState::Active);
        // went back by itself, with why
        let v = login_view(&json!({"checked": true, "dm": "sddm", "boot": "b2",
                                   "record": {"previous": "sddm", "state": "reverted", "reason": "the login screen didn't appear", "boot": "b2"}}));
        assert_eq!((v.state, v.why.as_str(), v.dm.as_str()), (LoginState::Reverted, "the login screen didn't appear", "SDDM"));
        // went back by choice, before the restart
        let v = login_view(&json!({"checked": true, "dm": "sddm", "boot": "b2", "record": {"previous": "sddm", "state": "off", "boot": "b2"}}));
        assert_eq!((v.state, v.off_pending), (LoginState::Other, true));
        // greetd with another greeter isn't Tessera's
        assert_eq!(login_view(&json!({"checked": true, "dm": "greetd", "tessera": false})).state, LoginState::Other);
        // an install from before the record: install.sh's note is the way back
        let v = login_view(&json!({"checked": true, "dm": "greetd", "tessera": true, "hint": "ly"}));
        assert_eq!((v.state, v.before.as_str()), (LoginState::Active, "ly"));
        assert_eq!(login_view(&json!({"checked": true, "dm": "greetd", "tessera": true})).before, "");
        // not read yet: what the copy says, never the switch
        assert_eq!(login_view(&json!({"installed": true})).state, LoginState::Active);
        assert_eq!(login_view(&json!({})).state, LoginState::Checking);
        assert_eq!(login_view(&json!({"checked": true, "dm": "sddm", "error": "No."})).error, "No.");
    }

    #[test]
    fn six_faces() {
        assert_eq!(FACES[4].0, "analog");
    }
}
