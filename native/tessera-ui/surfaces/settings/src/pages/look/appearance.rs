//! Appearance (pages/appearance.qml) and its Glass subpage
//! (pages/appearance-glass.qml; "Match apps" in Material), with the look
//! previews (shell/common/LookPreview.qml, drawn in settings-look.slint).

use super::super::super::model::{color, Ctx, Opt, Page, Row};
use super::super::keys;
use super::{amounts, feed, idx, palette, time_short, wallpaper_path};
use serde_json::{json, Value};

/// "6:30 AM" or "06:30" for minutes since midnight (Focus.fmt, TimeFormat.hm)
pub fn hm(minutes: i64, h24: bool) -> String {
    let (h, m) = ((minutes / 60).rem_euclid(24), minutes.rem_euclid(60));
    if h24 {
        format!("{h:02}:{m:02}")
    } else {
        format!("{}:{m:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
    }
}

/// The two looks with the preview's data: [glass, material, palette…]
fn look_data(cx: &Ctx) -> Vec<Opt> {
    let look = cx.s("appearance.look");
    let mut d = vec![Opt::new("Glass").id("glass").on(look != "material"), Opt::new("Material").id("material").on(look == "material")];
    d.extend(palette(cx));
    d
}

fn choose_look(cx: &mut Ctx, look: &str) {
    if cx.s("appearance.look") == look {
        return;
    }
    // (the shell sets it and restarts in that look behind the switching screen)
    cx.ask("look", "switchLook", json!([look]));
}

/// The installed icon themes (not cursor themes), as Appearance lists them
const ICON_THEMES: &str = "for d in \"${XDG_DATA_HOME:-$HOME/.local/share}/icons\" \"$HOME/.local/share/icons\" \"$HOME/.icons\" /usr/local/share/icons /usr/share/icons; do for t in \"$d\"/*/index.theme; do [ -f \"$t\" ] && grep -q '^Directories=' \"$t\" && basename \"$(dirname \"$t\")\"; done; done | grep -vx hicolor | sort -u";

/// The menu icon's choices: (value, icon, name)
pub const MENU_ICONS: [(&str, &str, &str); 18] = [
    ("tessera", "circle-wavy", "Badge"),
    ("distro", "", ""),
    ("linux", "linux-logo", "Linux"),
    ("command", "command", "Command"),
    ("terminal", "terminal-window", "Terminal"),
    ("cpu", "cpu", "Chip"),
    ("atom", "atom", "Atom"),
    ("planet", "planet", "Planet"),
    ("rocket", "rocket-launch", "Rocket"),
    ("star", "star-four", "Sparkle"),
    ("lightning", "lightning", "Bolt"),
    ("flame", "flame", "Flame"),
    ("leaf", "leaf", "Leaf"),
    ("snowflake", "snowflake", "Snowflake"),
    ("moon", "moon-stars", "Night"),
    ("cat", "cat", "Cat"),
    ("ghost", "ghost", "Ghost"),
    ("alien", "alien", "Alien"),
];

/// /etc/os-release's name and logo (SystemInfo.osName, .osLogo)
pub fn os_release() -> (String, String) {
    let text = std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).unwrap_or_default();
    let field = |k: &str| -> String {
        text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim().trim_matches('"').to_string()).unwrap_or_default()
    };
    let name = [field("PRETTY_NAME"), field("NAME")].into_iter().find(|s| !s.is_empty()).unwrap_or_else(|| "Linux".into());
    (name, field("LOGO"))
}

pub fn appearance(cx: &mut Ctx, p: &mut Page) {
    let h24 = cx.b("appearance.clock24");
    let wall = wallpaper_path(cx);
    let wall_img = if wall.is_empty() { None } else { cx.picture(&wall, 640, 400) };
    let look = cx.s("appearance.look");
    let choose = cx.on(|cx, a| {
        let l = if a.i == 1 { "material" } else { "glass" };
        choose_look(cx, l);
    });

    // Material: two large preview cards across the page
    if cx.material {
        let d = look_data(cx);
        let b = p.custom("look", "look-cards");
        b.data(d).id(choose.clone()).title("Look").text("Material (experimental), after Google's Material 3 Expressive: tonal colour from your wallpaper, bold shapes, springy motion. Switching restarts the shell; apps keep running.");
        b.footer = time_short(h24);
        if let Some(i) = wall_img.clone() {
            b.image(i);
        }
    }

    let mut rows = Vec::new();
    // Glass: small thumbnails, when the Material look's files are there
    if !cx.material && crate::util::asset("material/shell.qml").is_some() {
        let mut r = Row::new("Look")
            .detail(if look == "material" {
                "Material (experimental), after Google's Material 3 Expressive. Switching restarts the shell; apps keep running."
            } else {
                "Glass. Material (experimental) follows Google's Material 3 Expressive; switching restarts the shell, apps keep running."
            })
            .ctl("look", "look-thumbs", choose.clone())
            .data(look_data(cx))
            .text_value(time_short(h24));
        if let Some(i) = wall_img.clone() {
            r = r.image(i);
        }
        rows.push(r);
    }

    let mode = cx.s("appearance.mode");
    let modes = ["light", "dark", "auto"];
    let detail = if mode == "auto" {
        let (rise, set) = (feed(cx)["sunrise"].as_i64().unwrap_or(390), feed(cx)["sunset"].as_i64().unwrap_or(1110));
        format!("Light from sunrise ({}) to sunset ({})", hm(rise, h24), hm(set, h24))
    } else {
        String::new()
    };
    let mp = cx.pick("appearance.mode", &modes);
    rows.push(Row::new("Appearance").detail(detail).segmented(&["Light", "Dark", "Auto"], idx(&modes, &mode), mp).width(260.0));

    let t24 = cx.toggle("appearance.clock24");
    rows.push(Row::new("24-hour time").detail(format!("Clocks, events and alarms show {}", if h24 { "17:30" } else { "5:30 PM" })).switch(h24, t24));

    let style = cx.s("appearance.iconStyle");
    let (values, options, w): (Vec<&str>, Vec<&str>, f32) = if cx.material {
        (vec!["default", "themed", "dark", "clear", "tinted", "auto"], vec!["Default", "Themed", "Dark", "Clear", "Tinted", "Auto"], 400.0)
    } else {
        (vec!["default", "dark", "clear", "tinted", "auto"], vec!["Default", "Dark", "Clear", "Tinted", "Auto"], 340.0)
    };
    let shown = if style == "look" { if cx.material { "themed" } else { "clear" } } else { style.as_str() };
    let detail = match style.as_str() {
        "look" => if cx.material { "Your apps' own logos in your colours, like Android's themed icons" } else { "Your apps' own logos, frosted, like iOS's clear icons" },
        "themed" => if cx.material { "In your colours, like Android's themed icons" } else { "" },
        "dark" => "Dark backgrounds, colourful artwork",
        "clear" => "Frosted and colourless",
        "tinted" => "Tinted with the accent colour",
        "auto" => "Dark icons in dark mode",
        _ => "",
    };
    let ip = cx.pick("appearance.iconStyle", &values);
    rows.push(Row::new("App icons").detail(detail).segmented(&options, idx(&values, shown), ip).width(w));

    if cx.material {
        // Seeds: Material's baseline purple and a hue wheel (right to left, as the QML's Flow)
        let seeds = ["wallpaper", "#6750A4", "#B3261E", "#C26A00", "#8A7D00", "#386A20", "#006A6A", "#0061A4", "#7D5260"];
        let chosen = cx.s("appearance.materialSeed");
        let wall_seed = feed(cx)["seed"].as_str().unwrap_or("#6750A4").to_string();
        let rev: Vec<&str> = seeds.iter().rev().copied().collect();
        let sw: Vec<Opt> = rev
            .iter()
            .map(|s| {
                let o = Opt::new(if *s == "wallpaper" { "Wallpaper" } else { *s }).on(chosen == *s).color(color(if *s == "wallpaper" { &wall_seed } else { s }));
                if *s == "wallpaper" { o.icon("image") } else { o }
            })
            .collect();
        let pick = cx.pick("appearance.materialSeed", &rev);
        rows.push(Row::new("Colours from").detail(if chosen == "wallpaper" { "Your wallpaper, like Material You" } else { "This colour" }).swatches(sw, pick));

        let styles = ["tonalSpot", "vibrant", "expressive", "neutral"];
        let st = cx.s("appearance.materialStyle");
        let sp = cx.pick("appearance.materialStyle", &styles);
        rows.push(Row::new("Colour style")
            .detail(match st.as_str() {
                "tonalSpot" => "Calm: one colour with soft companions",
                "vibrant" => "Saturated colours throughout",
                "expressive" => "Playful: contrasting companion colours",
                "neutral" => "Nearly grey, a hint of colour",
                _ => "",
            })
            .segmented(&["Tonal", "Vibrant", "Expressive", "Neutral"], idx(&styles, &st), sp)
            .width(340.0));

        // (one control, its left end solid: Reduce transparency shows as solid)
        let reduce = cx.b("appearance.reduceTransparency");
        let tr = cx.shell.look.transparency;
        let detail = if reduce {
            "Solid: Reduce transparency is on. Slide to let the desktop show through.".to_string()
        } else if tr > 0.0 {
            format!("{}%: panels, menus, tiles and buttons show the blurred desktop behind them", (tr * 100.0).round())
        } else {
            "Solid. Slide to let the blurred desktop show through panels, menus, tiles and buttons".to_string()
        };
        let tp = cx.on(|cx, a| {
            cx.set_num("appearance.materialTransparency", (a.f as f64 * 20.0).round() / 20.0);
            if a.f > 0.0 {
                cx.set("appearance.reduceTransparency", false);
            }
        });
        rows.push(Row::new("Transparency").detail(detail).slider(if reduce { 0.0 } else { cx.f("appearance.materialTransparency") }, 0.0, 1.0, 0.0, tp).width(220.0));
    } else {
        let names = ["wallpaper", "graphite", "green", "yellow", "orange", "red", "pink", "purple", "blue"];
        let chosen = cx.s("appearance.accent");
        let wall_accent = feed(cx)["accent"].as_str().map(str::to_string).unwrap_or_else(|| crate::theme::accent_hex("blue", cx.dark));
        let rev: Vec<&str> = names.iter().rev().copied().collect();
        let dark = cx.dark;
        let sw: Vec<Opt> = rev
            .iter()
            .map(|n| {
                let c = if *n == "wallpaper" { wall_accent.clone() } else { crate::theme::accent_hex(n, dark) };
                let o = Opt::new(*n).on(chosen == *n).color(color(&c));
                if *n == "wallpaper" { o.icon("image") } else { o }
            })
            .collect();
        let pick = cx.pick("appearance.accent", &rev);
        rows.push(Row::new("Accent colour").detail(if chosen == "wallpaper" { "Following the wallpaper" } else { "" }).swatches(sw, pick));
    }
    let g = p.group("");
    for r in rows {
        g.row(r);
    }

    let gm = super::super::glass_material(cx);
    let value = match gm.as_str() {
        "liquid" => "Glass",
        "frosted" => "Frost",
        "solid" => "Solid",
        "custom" => "Custom",
        _ => "",
    };
    let l1 = cx.link("appearance/glass", "");
    let l2 = cx.link("appearance/glass", "");
    p.group("")
        .row(Row::link("Glass", "appearance/glass").visible(!cx.material).icon("drop-half").detail("Material, frost, lens and which apps match it").value(value).id(l1))
        .row(Row::link("Match apps", "appearance/glass").visible(cx.material).icon("paint-brush").detail("Colour GTK and Qt apps, terminals and browsers to match").id(l2));

    let reduce = cx.b("appearance.reduceTransparency");
    let rt = cx.toggle("appearance.reduceTransparency");
    let motion = cx.b("appearance.reduceMotion");
    let rm = cx.toggle("appearance.reduceMotion");
    let level = cx.shell.look.contrast.min(2) as usize;
    let cp = cx.on(|cx, a| {
        let v = ["standard", "higher", "maximum"];
        if let Some(x) = v.get(a.i.max(0) as usize) {
            cx.set("appearance.contrast", *x);
            cx.set("appearance.increaseContrast", false); // (the old switch)
        }
    });
    p.group("Accessibility")
        .row(Row::new("Reduce transparency").visible(!cx.material).detail("Solid surfaces instead of glass, whatever the material above").switch(reduce, rt))
        .row(Row::new("Reduce motion").detail("Fades instead of movement: island alerts, connecting a display (windows move without gliding) and the morning screen").switch(motion, rm))
        .row(Row::new("Text contrast")
            .detail(["Light secondary text, as on macOS", "Easier-to-read text, icons and controls", "The clearest text, icons and controls"][level])
            .segmented(&["Standard", "Higher", "Maximum"], level as i32, cp)
            .width(280.0));

    // ── More options ──
    p.more(cx);
    {
        let istyle = cx.s("appearance.iconStyle");
        let ic = cx.num("appearance.iconContrast", 0.01);
        let icv = cx.f("appearance.iconContrast");
        // Emoji: the font, and Install when it's missing
        let estyle = cx.s("emoji.style");
        let evalues = ["android", "apple", "windows"];
        let available = feed(cx)["emojiAvailable"].as_bool().unwrap_or(true);
        let pkg = feed(cx)["emojiPackage"].as_str().unwrap_or("").to_string();
        let ep = cx.on(move |cx, a| {
            if a.i == 100 {
                let s = cx.s("emoji.style");
                cx.ask("look", "installEmoji", json!([s]));
            } else if let Some(v) = evalues.get(a.i.max(0) as usize) {
                cx.set("emoji.style", *v);
            }
        });
        let mut emoji = Row::new("Emoji")
            .detail(if available { "In the emoji picker (Super+.) and Search".to_string() } else { format!("{pkg} isn't installed; showing Noto until it is") })
            .ctl("look", "emoji", ep)
            .text_value("Install font");
        emoji.on = available;
        emoji.opts = ["Noto", "Apple", "Fluent"].iter().map(|o| Opt::new(*o)).collect();
        emoji.index = idx(&evalues, &estyle);
        p.group("")
            .row(Row::new("Icon contrast")
                .visible(istyle == "clear" || istyle == "tinted")
                .detail("How clearly each logo stands out from its tile; every logo is also evened out on its own")
                .slider(icv, 0.0, 1.0, 0.0, ic)
                .width(220.0))
            .row(emoji);

        // Menu icon, icon theme, shortcut symbols
        let (os_name, os_logo) = os_release();
        let menu_icon = cx.s("appearance.menuIcon");
        let logo = if os_logo.is_empty() { None } else { super::theme_icon(&[os_logo.as_str()]) };
        let icons: Vec<Opt> = MENU_ICONS
            .iter()
            .map(|(v, icon, name)| {
                let on = menu_icon == *v || (*v == "tessera" && (menu_icon == "circle" || menu_icon == "irice"));
                let name = if *v == "distro" { os_name.clone() } else { name.to_string() };
                let mut o = Opt::new(name).icon(if icon.is_empty() { "linux-logo" } else { icon }).on(on).id(*v);
                if *v == "distro" {
                    if let Some(i) = logo.clone() {
                        o = o.image(i);
                    }
                }
                o
            })
            .collect();
        let mp = cx.on(|cx, a| {
            if let Some((v, _, _)) = MENU_ICONS.get(a.i.max(0) as usize) {
                cx.set("appearance.menuIcon", *v);
            }
        });
        let themes: Vec<String> = cx
            .once("look.iconThemes", || Value::from(Ctx::run(&["sh", "-c", ICON_THEMES])))
            .as_str()
            .unwrap_or("")
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        let mut opts = vec!["Automatic".to_string()];
        opts.extend(themes.iter().cloned());
        let cur = cx.s("appearance.iconTheme");
        let index = if cur.is_empty() { 0 } else { themes.iter().position(|t| *t == cur).map_or(-1, |i| i as i32 + 1) };
        let tp = cx.on(move |cx, a| {
            let v = if a.i <= 0 { String::new() } else { themes.get(a.i as usize - 1).cloned().unwrap_or_default() };
            if v == cx.s("appearance.iconTheme") {
                return;
            }
            cx.set("appearance.iconTheme", v);
            cx.ask("look", "repairShell", Value::Null); // (after the setting is saved)
        });
        let ks = cx.s("appearance.keySymbols");
        let kvals = ["auto", "mac", "windows", "linux"];
        let kp = cx.pick("appearance.keySymbols", &kvals);
        let example = keys::combo(&*cx.cfg, "SUPER CTRL F");
        p.group("Menu bar and shortcuts")
            .footer(format!(
                "The icon opens the system menu. Shortcut symbols are how keys are written in menus, Settings and the cheatsheet{}",
                if ks == "auto" { "; Auto follows the icon: Command uses ⌘ ⌃ ⌥ ⇧, others Super Ctrl Alt." } else { "." }
            ))
            .row(Row::new("Menu icon").ctl("look", "menu-icons", mp).data(icons))
            .row(Row::new("Icon theme")
                .detail("App icons in the dock, App Drawer, Search and Overview. Changing it restarts the shell; apps keep running.")
                .choice(&opts, index, tp)
                .width(220.0))
            .row(Row::new("Shortcut symbols").detail(format!("e.g. {example}")).segmented(&["Auto", "⌘ macOS", "Windows", "Linux"], idx(&kvals, &ks), kp).width(300.0));
    }
    p.end_more();
}

/// The custom material's and the optics' sliders: (key, label, hint, from, to)
const ADVANCED: [(&str, &str, &str, f32, f32); 7] = [
    ("glassLens", "Glass", "How much it bends what's behind", 0.0, 1.0),
    ("glassFrost", "Frost", "How much it blurs", 0.0, 1.0),
    ("glassSolid", "Solid", "How opaque it is", 0.0, 1.0),
    ("glassTint", "Tint", "", 0.0, 2.0),
    ("glassIor", "Index of refraction", "How strongly the edge bends what's behind; glass is about 1.5", 1.0, 2.0),
    ("glassRefraction", "Refraction", "How far in from the edge the bending reaches", 0.4, 2.0),
    ("glassDispersion", "Dispersion", "Colour fringes at the edge, like a prism", 0.0, 1.0),
];

/// A slider's reading in the Advanced panel ("75%", "1.50", "None")
pub fn advanced_value(key: &str, v: f32) -> String {
    if key == "glassIor" {
        format!("{v:.2}")
    } else if key == "glassDispersion" && v < 0.005 {
        "None".into()
    } else {
        format!("{}%", (v * 100.0).round())
    }
}

/// Frost over windows: the nearest of its four steps
pub fn nearest(values: &[f32], v: f32) -> i32 {
    let mut best = 0;
    for (i, x) in values.iter().enumerate() {
        if (x - v).abs() < (values[best] - v).abs() {
            best = i;
        }
    }
    best as i32
}

pub fn glass(cx: &mut Ctx, p: &mut Page) {
    p.title = if cx.material { "Match apps".into() } else { "Glass".into() };

    // Material has no glass, so this page keeps only Match apps, and says why
    if cx.material {
        let show = cx.on(|cx, _| cx.go("appearance", ""));
        p.group("").row(Row::new("Glass belongs to the Glass look")
            .icon("info")
            .detail("Material's surfaces are solid colours from your wallpaper, so there's no glass to tune. Your glass settings are kept for when you switch back.")
            .button("Show looks", show));
    }

    let gm = super::super::glass_material(cx);
    let [lens, _frost, solid, _tint] = amounts(cx);
    let tier = super::super::glass_tier(cx);
    let plugin = feed(cx)["glassPlugin"].as_bool().unwrap_or(cx.shell.plugin);
    let version = feed(cx)["glassPluginVersion"].as_i64().unwrap_or(0);
    let advanced = cx.local_b("look.glassAdvanced");
    if !cx.material {
        let custom = gm == "custom";
        let values = ["liquid", "frosted", "solid", "custom"];
        let options: Vec<&str> = if custom { vec!["Glass", "Frost", "Solid", "Custom"] } else { vec!["Glass", "Frost", "Solid"] };
        let mat = cx.on(move |cx, a| {
            if a.i == 100 {
                // The three amounts behind the presets, as sliders; starting
                // from whatever is showing, so they don't jump
                let open = !cx.local_b("look.glassAdvanced");
                if open {
                    let a = amounts(cx);
                    cx.set_num("appearance.glassLens", a[0] as f64);
                    cx.set_num("appearance.glassFrost", a[1] as f64);
                    cx.set_num("appearance.glassSolid", a[2] as f64);
                    cx.set_num("appearance.glassTint", a[3] as f64);
                }
                cx.local_set("look.glassAdvanced", open);
            } else if let Some(v) = values.get(a.i.max(0) as usize) {
                cx.set("appearance.glass", *v);
            }
        });
        let mut material = Row::new("Material").ctl("look", "glass-material", mat).width(if custom { 300.0 } else { 230.0 });
        material.opts = options.iter().map(|o| Opt::new(*o)).collect();
        material.index = idx(&values, &gm);
        material.on = advanced;
        let footer = if lens > 0.5 && !plugin {
            "Glass needs the tessera-glass Hyprland plugin; until it's built, it looks frosted."
        } else {
            match gm.as_str() {
                "liquid" => "Glass bends whatever is behind it, live, with light catching the edges.",
                "frosted" => "Frost (the default) blurs what's behind, with a thin lens at the edges: light on the GPU. Glass is the full engine, bending what's behind it live.",
                "solid" => "Solid turns transparency off, everywhere.",
                _ => "Your own mix of lens, frost and solid. Advanced… changes it.",
            }
        };
        let motion = cx.s("appearance.motion");
        let mv = ["subtle", "snappy", "expressive"];
        let mp = cx.pick("appearance.motion", &mv);
        let radius = cx.i("appearance.windowRadius");
        let rn = cx.num("appearance.windowRadius", 1.0);
        let corners = cx.b("appearance.screenCorners");
        let ct = cx.toggle("appearance.screenCorners");
        p.group("Glass")
            .footer(footer)
            .row(material)
            .row(Row::new("Motion")
                .detail(match motion.as_str() {
                    "expressive" => "More pull toward the pointer and bouncier springs, like iOS 26",
                    "snappy" => "Quick springs that settle at once, with little pull: the quickest feel",
                    _ => "Springs with a touch of overshoot, like macOS",
                })
                .segmented(&["Subtle", "Snappy", "Expressive"], idx(&mv, &motion), mp)
                .width(300.0))
            .row(Row::new("Window corners").detail(format!("{radius} px, continuous curve")).slider(radius as f32, 0.0, 32.0, 1.0, rn).width(220.0))
            .row(Row::new("Rounded screen corners").switch(corners, ct));

        // Advanced…: the custom material and the optics (the QML's popover,
        // here under the group)
        if advanced {
            let dark = cx.dark;
            let data: Vec<Opt> = ADVANCED
                .iter()
                .enumerate()
                .map(|(i, (key, label, hint, _, _))| {
                    // (each slider's range: settings-look.slint's by `style`, ADVANCED's)
                    let v = cx.f(&format!("appearance.{key}"));
                    let hint = if *key == "glassTint" { if dark { "How much it darkens what's behind" } else { "How milky it is" } } else { hint };
                    Opt::new(*label).detail(hint).num(v).style(i as i32).id(advanced_value(key, v))
                })
                .collect();
            let act = cx.on(|cx, a| {
                if a.i == 200 {
                    // Reset to Glass
                    cx.set("appearance.glass", "liquid");
                    cx.set_num("appearance.glassIor", 1.5);
                    cx.set_num("appearance.glassRefraction", 1.0);
                    cx.set_num("appearance.glassDispersion", 0.35);
                    cx.set_num("appearance.glassLens", 1.0);
                    cx.set_num("appearance.glassFrost", 0.75);
                    cx.set_num("appearance.glassSolid", 0.0);
                    cx.set_num("appearance.glassTint", 0.75);
                    return;
                }
                let Some((key, ..)) = ADVANCED.get(a.i.max(0) as usize) else { return };
                cx.set_num(&format!("appearance.{key}"), (a.f as f64 * 100.0).round() / 100.0);
                // (the amounts make it Custom; the optics apply to any material)
                if a.i < 4 {
                    cx.set("appearance.glass", "custom");
                }
            });
            let b = p.custom("look", "glass-advanced");
            b.data(data).id(act).above(8.0);
            b.num = lens.max(0.15);
        }
    }

    // Match apps
    let available = cx.material || feed(cx)["appThemeAvailable"].as_bool().unwrap_or(false);
    let at = cx.s("appearance.appTheme");
    let av = ["off", "accent", "wallpaper"];
    let ap = cx.pick("appearance.appTheme", &av);
    let footer = if !available {
        "Install matugen to colour your apps to match.".to_string()
    } else if at == "off" {
        "Apps keep their own colours.".to_string()
    } else {
        format!(
            "Apps get colours from your {}, light or dark with Tessera. Open apps update when they can; others on their next start. Turning this off removes the colours again.",
            if at == "wallpaper" { "wallpaper" } else { "accent colour" }
        )
    };
    let mut rows = vec![Row::new("Colours from").segmented(&["Off", "Accent", "Wallpaper"], idx(&av, &at), ap).enabled(available).width(260.0)];
    if at != "off" {
        let targets = cx.list("appearance.appThemeTargets");
        for (id, label) in [("gtk", "GTK apps"), ("qt", "Qt and KDE apps"), ("terminals", "Terminals (foot, kitty, Alacritty)"), ("browsers", "Firefox (with Pywalfox) and Chromium")] {
            let on = targets.iter().any(|t| t == id);
            let t = cx.on(move |cx, _| {
                let cur = cx.list("appearance.appThemeTargets");
                let mut next: Vec<Value> = cur.iter().filter(|x| *x != id).map(|x| Value::from(x.clone())).collect();
                if !cur.iter().any(|x| x == id) {
                    next.push(Value::from(id));
                }
                cx.set("appearance.appThemeTargets", Value::Array(next));
            });
            rows.push(Row::new(label).switch(on, t));
        }
    }
    let g = p.group("Match apps");
    g.footer(footer);
    for r in rows {
        g.row(r);
    }

    // ── More options ──
    p.more(cx);
    {
        let depth = cx.s("appearance.glassDepth");
        let dv = ["thin", "depth", "auto"];
        let dp = cx.pick("appearance.glassDepth", &dv);
        let outdated = plugin && (version == 3 || version == 4);
        let build = cx.on(|cx, _| cx.ask("look", "buildPlugin", Value::Null));
        let mut glass_row = Row::new("Compositor glass").visible(tier != "solid").detail(if outdated {
            "An older build. Update it so glass never shows over full-screen apps."
        } else if plugin {
            "Active: Hyprland draws the glass"
        } else {
            "Not loaded. Build it once (and again after Hyprland updates)."
        });
        glass_row = if !plugin || outdated { glass_row.button(if outdated { "Update plugin…" } else { "Build plugin…" }, build).primary() } else { glass_row.ctl("look", "check", String::new()) };
        let tint = cx.b("appearance.wallpaperTint");
        let tt = cx.toggle("appearance.wallpaperTint");
        let frost_values = [0.0f32, 0.5, 0.75, 0.95];
        let fp = cx.on(move |cx, a| {
            if let Some(v) = frost_values.get(a.i.max(0) as usize) {
                cx.set_num("appearance.glassWindowFrost", *v as f64);
            }
        });
        let wf = cx.f("appearance.glassWindowFrost");
        p.group("Glass")
            .row(Row::new("Lens").visible(lens > 0.4 && solid < 0.9).segmented(&["Thin", "Depth", "Auto"], idx(&dv, &depth), dp).width(260.0))
            .row(glass_row)
            .row(Row::new("Tint glass with wallpaper colour").switch(tint, tt))
            .row(Row::new("Frost over windows")
                .visible(plugin && tier != "solid")
                .detail("Glass covering a window blurs it more, so its text doesn't compete with what's on the glass")
                .segmented(&["Off", "Subtle", "Medium", "Strong"], nearest(&frost_values, wf), fp)
                .width(300.0));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_as_the_shell_writes_them() {
        assert_eq!(hm(390, false), "6:30 AM");
        assert_eq!(hm(1110, false), "6:30 PM");
        assert_eq!(hm(0, false), "12:00 AM");
        assert_eq!(hm(390, true), "06:30");
    }

    #[test]
    fn advanced_readings() {
        assert_eq!(advanced_value("glassIor", 1.5), "1.50");
        assert_eq!(advanced_value("glassDispersion", 0.0), "None");
        assert_eq!(advanced_value("glassFrost", 0.75), "75%");
        assert_eq!(nearest(&[0.0, 0.5, 0.75, 0.95], 0.8), 2);
        assert_eq!(nearest(&[0.0, 0.5, 0.75, 0.95], 0.0), 0);
    }

    #[test]
    fn eighteen_menu_icons() {
        assert_eq!(MENU_ICONS.len(), 18);
        assert_eq!(MENU_ICONS[0].0, "tessera");
    }
}
