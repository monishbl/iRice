//! Battery & Power (pages/power.qml) and Cool-down and performance
//! (power-cooldown.qml). The energy mode, who decided it, the battery, the
//! apps' energy and Cool-down's state are the shell's (feed.system.power,
//! feed.system.cooldown); switching the mode, Caffeine, the shell's
//! priority, quitting an app and Cool-down go through it (cx.ask). The
//! idle, lid and password settings are config.json's, applied by the shell.

use super::super::super::model::{btn, Ctx, Page, Row};
use serde_json::{json, Value};

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// The idle timers' choices (power.qml's Seconds): (seconds, label)
pub fn seconds_options(never: bool) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    if never {
        out.push((0, "Never".to_string()));
    }
    for s in [60, 120, 180, 300, 600, 900, 1800, 3600] {
        out.push((s, if s < 3600 { format!("{} {}", s / 60, if s == 60 { "minute" } else { "minutes" }) } else { "1 hour".into() }));
    }
    out
}

/// "12 min of CPU" or "40 s of CPU"
pub fn cpu_time(seconds: f64) -> String {
    let min = (seconds / 60.0).round();
    if min >= 1.0 {
        format!("{min} min of CPU")
    } else {
        format!("{} s of CPU", seconds.round())
    }
}

/// A timer row: a Dropdown of durations for `power.<key>`
fn seconds_row(cx: &mut Ctx, label: &str, detail: &str, key: &str) -> Row {
    let path = format!("power.{key}");
    let opts = seconds_options(true);
    let v = cx.i(&path);
    // (a value not in the list shows itself, as Dropdown's String(value))
    let mut labels: Vec<String> = opts.iter().map(|o| o.1.clone()).collect();
    let mut values: Vec<Value> = opts.iter().map(|o| Value::from(o.0)).collect();
    let index = match opts.iter().position(|o| o.0 == v) {
        Some(i) => i as i32,
        None => {
            labels.push(v.to_string());
            values.push(Value::from(v));
            labels.len() as i32 - 1
        }
    };
    let id = cx.pick_values(&path, values);
    Row::new(label).detail(detail).choice(&labels, index, id).width(160.0)
}

/// The lid's choice (power.qml's Lid)
fn lid_row(cx: &mut Ctx, label: &str, detail: &str, key: &str) -> Row {
    let path = format!("power.{key}");
    let values = ["sleep", "lock", "nothing"];
    let v = cx.s(&path);
    let id = cx.pick(&path, &values);
    Row::new(label).detail(detail).segmented(&["Sleep", "Lock", "Do nothing"], values.iter().position(|x| *x == v).unwrap_or(0) as i32, id).width(260.0)
}

/// Battery & Power (power.qml)
pub fn power(cx: &mut Ctx, p: &mut Page) {
    let feed = cx.feed;
    let pw = &feed["system"]["power"];
    let ppd = pw["ppd"].as_bool().unwrap_or(false);
    let battery = pw["hasBattery"].as_bool().unwrap_or(false);
    let perf = pw["hasPerformance"].as_bool().unwrap_or(false);

    // ── Energy mode ──
    let mut rows = Vec::new();
    if !ppd {
        rows.push(Row::new(s(&pw["powerShort"])).detail(s(&pw["powerReason"])));
    } else {
        let why = s(&pw["why"]);
        let values = ["power-saver", "balanced", "performance"];
        let mode = s(&pw["mode"]);
        let id = cx.on(move |cx, a| {
            if let Some(v) = values.get(a.i.max(0) as usize) {
                cx.ask("system", "powerMode", json!([v]));
            }
        });
        let options: &[&str] = if perf { &["Low Power", "Balanced", "Performance"] } else { &["Low Power", "Balanced"] };
        rows.push(
            Row::new("Energy mode")
                .detail(if !why.is_empty() {
                    format!("Now {} · {why}", s(&pw["modeName"]))
                } else if perf {
                    String::new()
                } else {
                    "This computer has no Performance mode".into()
                })
                .segmented(options, values.iter().position(|v| *v == mode).unwrap_or(0) as i32, id)
                .width(if perf { 330.0 } else { 220.0 }),
        );
    }
    if pw["keepAwake"].as_bool().unwrap_or(false) {
        let id = cx.on(|cx, _| cx.ask("system", "caffeineOff", Value::Null));
        rows.push(Row::new("Keeping awake").detail(format!("Caffeine is on · {}", s(&pw["keepAwakeStatus"]).to_lowercase())).button("Turn off Caffeine", id));
    }
    let g = p.group("Energy mode");
    g.footer(if ppd { "Cool-down comes first while the computer is hot, then the battery saver, then your choice or Game Mode, whichever came last." } else { "" });
    for r in rows {
        g.row(r);
    }

    // Cool-down (a subpage), and the shell's priority (where ananicy-cpp runs)
    let pr = &pw["priority"];
    let pr_on = pr["on"].as_bool().unwrap_or(false);
    let pr_id = cx.on(move |cx, _| cx.ask("system", "shellPriority", json!([!pr_on])));
    let pr_err = s(&pr["error"]);
    let link = cx.link("power/cooldown", "");
    p.group("")
        .row(Row::link("Cool-down and performance", "power/cooldown").icon("thermometer-simple").detail("What Tessera turns down when the machine runs hot, and what stays loaded").id(link))
        .row(
            Row::new("Keep Tessera smooth when busy")
                .visible(pr["available"].as_bool().unwrap_or(false))
                .detail(if pr_err.is_empty() { "Panels and animations keep their speed while apps work hard. Asks ananicy-cpp to treat the shell as interactive.".to_string() } else { pr_err })
                .switch(pr_on, pr_id)
                .enabled(!pr["busy"].as_bool().unwrap_or(false)),
        );

    // ── Battery ──
    if battery {
        let crit = cx.b("power.critical");
        let t = cx.toggle("power.critical");
        let guard = s(&pw["guardError"]);
        p.group("Battery")
            .footer(if guard.is_empty() { "It sleeps rather than shutting down, so your work is there when you plug in. This keeps working even if Tessera isn't running.".to_string() } else { guard })
            .row(Row::new("Critical battery").detail("Warn at 10% and 5%, sleep at 3% (even with Caffeine on)").switch(crit, t));
    }

    // ── Idle ──
    let own = pw["ownHypridle"].as_bool().unwrap_or(false);
    if !own {
        let k = |a: &str, b: &str| if battery { a.to_string() } else { b.to_string() };
        let rows = vec![
            seconds_row(cx, "Dim the screen after", "", &k("idleDimBattery", "idleDimAc")),
            seconds_row(cx, "Turn the screen off after", "", &k("idleOffBattery", "idleOffAc")),
            seconds_row(cx, "Lock after", "The password is needed from then on", &k("idleLockBattery", "idleLockAc")),
            seconds_row(cx, "Sleep after", "", &k("idleSleepBattery", "idleSleepAc")),
        ];
        let g = p.group(if battery { "On battery" } else { "When you're away" });
        for r in rows {
            g.row(r);
        }
        if battery {
            let rows = vec![
                seconds_row(cx, "Dim the screen after", "", "idleDimAc"),
                seconds_row(cx, "Turn the screen off after", "", "idleOffAc"),
                seconds_row(cx, "Lock after", "", "idleLockAc"),
                seconds_row(cx, "Sleep after", "", "idleSleepAc"),
            ];
            let g = p.group("On the charger");
            g.footer("Each step comes after the one before it. Apps that play video or calls keep the screen on.");
            for r in rows {
                g.row(r);
            }
        }
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        let file = s(&pw["userHypridle"]);
        let file = if !home.is_empty() { file.replacen(&home, "~", 1) } else { file };
        p.group("When you're away")
            .footer(format!("You have your own hypridle.conf ({file}), so its timers are used. Rename or remove it to set them here."))
            .row(Row::new("Using your own idle settings"));
    }

    // ── Lid and password ──
    if battery {
        let rows = vec![
            lid_row(cx, "Closing the lid on battery", "", "lidBattery"),
            lid_row(cx, "Closing the lid on the charger", "", "lidAc"),
            lid_row(cx, "With another display connected", "On the charger", "lidExternal"),
            lid_row(cx, "With Caffeine on", "Instead of sleeping", "lidCaffeine"),
        ];
        let after = cx.i("power.passwordAfterSleep");
        let values = [0i64, 60, 300];
        let id = cx.pick_values("power.passwordAfterSleep", values.iter().map(|v| Value::from(*v)).collect());
        let g = p.group("Lid and waking");
        g.footer("Lock and Do nothing keep the computer running with the lid closed (Wi-Fi, downloads and music carry on; the built-in screen goes off); with another display, it carries on there. A critically low battery still sleeps. If Tessera isn't running, closing the lid sleeps, as it always has.");
        for r in rows {
            g.row(r);
        }
        g.row(Row::new("Require the password after sleep").segmented(&["Immediately", "After 1 minute", "After 5 minutes"], values.iter().position(|v| *v == after).unwrap_or(0) as i32, id).width(330.0));
    }

    // ── More options ──
    p.more(cx);
    {
        let saver = cx.b("power.batterySaver");
        let auto = cx.b("power.batterySaverAuto");
        let level = cx.i("power.batterySaverLevel");
        let sid = cx.on(|cx, a| {
            if a.i > 0 {
                cx.set("power.batterySaverAuto", a.i == 2);
            }
            cx.set("power.batterySaver", a.i > 0);
        });
        let game = cx.b("power.gamePerformance");
        let gt = cx.toggle("power.gamePerformance");
        p.group("Energy mode")
            .row(
                Row::new("Low Power when the battery is low")
                    .visible(battery && ppd)
                    .detail(if !saver {
                        format!("At {level}% on battery, nothing changes")
                    } else if auto {
                        format!("At {level}% on battery, Low Power comes on by itself, and the island offers Undo")
                    } else {
                        format!("At {level}% on battery, once each time it runs down, the island asks")
                    })
                    .segmented(&["Off", "Ask", "Automatic"], if !saver { 0 } else if auto { 2 } else { 1 }, sid)
                    .width(240.0),
            )
            .row(
                Row::new("Game Mode uses Performance")
                    .visible(ppd)
                    .detail(if perf { "While Game Mode is on (Quick Settings)" } else { "This computer has no Performance mode" })
                    .switch(game, gt)
                    .enabled(perf),
            );

        // Apps using significant energy (the report runs while this page shows:
        // RustBridgeSystem.qml counts it in Power.energyUsers)
        let track = cx.b("power.appEnergy");
        let tt = cx.on(move |cx, _| {
            cx.set("power.appEnergy", !track);
            // (as the QML: a fresh start when it's turned on)
            if !track {
                cx.ask("system", "clearEnergy", Value::Null);
            }
        });
        let minutes = pw["energyMinutes"].as_i64().unwrap_or(0);
        let apps: Vec<(String, f64)> = pw["energyApps"].as_array().map(|a| a.iter().map(|x| (s(&x["key"]), x["seconds"].as_f64().unwrap_or(0.0))).collect()).unwrap_or_default();
        let shell = cx.shell;
        let entries = shell.get::<crate::services::apps::Apps>();
        cx.want("apps");
        let most = apps.first().map(|a| a.1).filter(|m| *m > 0.0).unwrap_or(1.0);
        let mut rows = vec![Row::new("Keep track").switch(track, tt)];
        if track && apps.is_empty() {
            rows.push(Row::new("Nothing has used much energy lately").detail("Apps show up here after they've kept the processor busy"));
        }
        if track {
            for (key, secs) in &apps {
                let (kind, ident) = key.split_once(':').map_or((key.as_str(), ""), |(k, i)| (k, i));
                let name = if kind == "proc" { None } else { entries.and_then(|e| e.entry_for(ident)).map(|e| e.name.clone()).filter(|n| !n.is_empty()) };
                let quit = if kind != "proc" {
                    let ident = ident.to_string();
                    Some(cx.on(move |cx, _| cx.ask("system", "quitApp", json!([ident.clone()]))))
                } else {
                    None
                };
                rows.push(
                    Row::new(name.unwrap_or_else(|| ident.to_string()))
                        .detail(cpu_time(*secs))
                        .ctl("system", "energy", quit.unwrap_or_default())
                        .num((secs / most) as f32)
                        .data(if kind != "proc" { vec![btn("Quit")] } else { Vec::new() }),
                );
            }
        }
        let g = p.group("Apps using significant energy");
        g.footer(if !track {
            "Off: nothing is recorded.".to_string()
        } else {
            format!("CPU time over the last {}. A look every 5 minutes; kept only on this computer.", if minutes > 0 && minutes < 60 { format!("{minutes} minutes") } else { "hour".into() })
        });
        for r in rows {
            g.row(r);
        }
    }
    p.end_more();
}

/// Cool-down and performance (power-cooldown.qml)
pub fn cooldown(cx: &mut Ctx, p: &mut Page) {
    p.title = "Cool-down and performance".into();
    let feed = cx.feed;
    let cd = &feed["system"]["cooldown"];
    let active = cd["active"].as_bool().unwrap_or(false);
    let why = s(&cd["why"]);
    let toggle = cx.on(|cx, _| cx.ask("system", "coolDownToggle", Value::Null));
    let hot = cx.b("cooling.whenHot");
    let action = cx.s("cooling.hotAction");
    let now = if !hot { "off" } else if action == "ask" { "ask" } else { "auto" };
    let values = ["off", "ask", "auto"];
    let hid = cx.on(move |cx, a| {
        let v = values[a.i.clamp(0, 2) as usize];
        if v != "off" {
            cx.set("cooling.hotAction", v);
        }
        cx.set("cooling.whenHot", v != "off");
    });
    let bat = cx.b("cooling.onBattery");
    let bt = cx.toggle("cooling.onBattery");
    p.group("Cool-down")
        .footer("Less work for the processor and graphics, so the laptop runs cooler and lasts longer: all of this comes back when cool-down ends. Also in Quick Settings.")
        .row(Row::new("Cool-down").detail(if active { format!("On · {why}") } else if !why.is_empty() { why.clone() } else { "Off".into() }).switch(active, toggle))
        .row(
            Row::new("When the CPU is hot")
                .detail(if !hot {
                    "At the island's temperature alert (Notifications & Island → Island)"
                } else if action == "ask" {
                    "The island asks first; the question stays until you answer. Not now keeps it off until the laptop cools."
                } else {
                    "Starts by itself; ends after 3 minutes well below the alert temperature"
                })
                .segmented(&["Off", "Ask", "Automatic"], values.iter().position(|v| *v == now).unwrap_or(0) as i32, hid)
                .width(240.0),
        )
        .row(Row::new("On battery").switch(bat, bt));

    p.more(cx);
    {
        let gvalues = ["keep", "frosted", "solid"];
        let glass = cx.s("cooling.glass");
        let gid = cx.pick("cooling.glass", &gvalues);
        let mut rows = vec![Row::new("Glass").visible(!cx.material).detail("During cool-down").segmented(&["Keep", "Frosted", "Solid"], gvalues.iter().position(|v| *v == glass).unwrap_or(0) as i32, gid).width(260.0)];
        for (key, label, detail, glass_only) in [
            ("lightBlur", "Lighter blur", "", true),
            ("shortAnimations", "Shorter animations", "", false),
            ("refreshDrop", "Lower refresh rate", "The laptop screen's lowest rate (48 Hz on most); apps draw fewer frames too. The screen goes black for a moment when it switches.", false),
            ("pauseLiveWallpaper", "Pause live wallpaper", "", false),
        ] {
            let path = format!("cooling.{key}");
            let on = cx.b(&path);
            let t = cx.toggle(&path);
            rows.push(Row::new(label).visible(!glass_only || !cx.material).detail(detail).switch(on, t));
        }
        let g = p.group("Cool-down");
        for r in rows {
            g.row(r);
        }
        let keep = cx.b("performance.keepPanelsLoaded");
        let kt = cx.toggle("performance.keepPanelsLoaded");
        p.group("Performance").row(
            Row::new("Keep panels loaded")
                .detail("Search, Quick Settings and Notification Center stay ready after their first open: faster and lighter on the processor, about 50 MB more memory")
                .switch(keep, kt),
        );
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_choices_as_the_qml_lists_them() {
        let o = seconds_options(true);
        assert_eq!(o[0], (0, "Never".to_string()));
        assert_eq!(o[1], (60, "1 minute".to_string()));
        assert_eq!(o[2], (120, "2 minutes".to_string()));
        assert_eq!(o.last().unwrap(), &(3600, "1 hour".to_string()));
        assert_eq!(seconds_options(false).len(), 8);
    }

    #[test]
    fn cpu_time_reads_naturally() {
        assert_eq!(cpu_time(20.0), "20 s of CPU");
        // (as the QML rounds: 40 s is "1 min")
        assert_eq!(cpu_time(40.0), "1 min of CPU");
        assert_eq!(cpu_time(720.0), "12 min of CPU");
    }
}
