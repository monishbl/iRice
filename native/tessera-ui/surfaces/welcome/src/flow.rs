//! The welcome screen's steps (shell/modules/welcome/Welcome.qml) as plain
//! logic: where you are, what each step says, the wallpapers offered and
//! which one is chosen, and what each choice asks of the shell. Nothing
//! here touches the system: every change (light or dark, a wallpaper, the
//! look, "done") is a request through `Ask`, which the part sends to the
//! QML shell (`shell::ask`: RustUi.qml, then Welcome.qml's own functions,
//! as the QML screen did them). Tests use a recorder in its place.

use serde_json::{json, Value};

pub const STEPS: usize = 4;

/// What a step asks of the shell (the part sends it; tests record it)
pub trait Ask {
    fn ask(&mut self, call: &'static str, args: Value);
}

/// Where you are in the welcome
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub step: usize,
    /// the look picked here (applied when you start)
    pub look: String,
    /// the look the settings have now
    pub current_look: String,
    /// Material's files are there (material/, beside the shell)
    pub material_available: bool,
    /// it was put away (Set up later, Start using Tessera)
    pub done: bool,
}

impl Flow {
    pub fn new(current_look: &str, material_available: bool) -> Self {
        Self { step: 0, look: current_look.to_string(), current_look: current_look.to_string(), material_available, done: false }
    }

    /// Continue (Enter): the next step, or on the last one start
    pub fn next(&mut self, a: &mut dyn Ask) {
        if self.step + 1 < STEPS {
            self.step += 1;
        } else {
            self.finish(a);
        }
    }

    /// Back (Esc)
    pub fn back(&mut self) {
        self.step = self.step.saturating_sub(1);
    }

    /// Set up later: put away for good (welcome.done)
    pub fn later(&mut self, a: &mut dyn Ask) {
        if !self.done {
            self.done = true;
            a.ask("welcome.later", json!([]));
        }
    }

    /// Start using Tessera: done, and the look switched if it changed (the
    /// shell runs switch-look.sh, as Settings → Appearance does)
    pub fn finish(&mut self, a: &mut dyn Ask) {
        if !self.done {
            self.done = true;
            a.ask("welcome.finish", json!([self.look]));
        }
    }

    pub fn pick_look(&mut self, look: &str) {
        if look == "glass" || (look == "material" && self.material_available) {
            self.look = look.to_string();
        }
    }

    /// Light, dark or auto (applies at once, as the QML)
    pub fn pick_mode(&self, a: &mut dyn Ask, mode: &str) {
        if ["light", "dark", "auto"].contains(&mode) {
            a.ask("welcome.mode", json!([mode]));
        }
    }

    /// One of the wallpapers offered (applies at once), or "own": a picture
    /// of your own through the shell's file picker
    pub fn pick_wall(&self, a: &mut dyn Ask, walls: &[Wall], id: &str) {
        if id == "own" {
            a.ask("welcome.pick", json!([]));
        } else if walls.iter().any(|w| w.id == id) {
            a.ask("welcome.wall", json!([id]));
        }
    }

    pub fn title(&self) -> &'static str {
        ["Welcome to Tessera", "Light or dark?", "Choose a wallpaper", "You're all set"][self.step.min(STEPS - 1)]
    }

    pub fn help(&self) -> &'static str {
        match self.step {
            0 => "Pick a look for your desktop. You can change it any time in Settings → Appearance.",
            1 => "Auto is light from sunrise to sunset and dark at night.",
            2 => "Pick one of these, or a picture of your own.",
            _ if self.look != self.current_look && self.look == "material" => "Tessera switches to Material when you start. Your apps keep running.",
            _ if self.look != self.current_look => "Tessera switches to Glass when you start. Your apps keep running.",
            _ => "Everything you picked can be changed in Settings.",
        }
    }

    /// The looks offered (Material only where its files are)
    pub fn looks(&self) -> Vec<&'static str> {
        if self.material_available { vec!["glass", "material"] } else { vec!["glass"] }
    }
}

/// A wallpaper offered (Welcome.qml's walls)
#[derive(Clone, Debug, PartialEq)]
pub struct Wall {
    pub id: String,
    pub name: String,
    pub preview: String,
    /// a still picture
    pub path: String,
    /// Material's pair
    pub light: String,
    pub dark: String,
}

/// The Tahoe set through the day, three of its pictures, and Material's
/// light and dark pair where it's there
pub fn walls(tahoe: &str, material: &str, dark: bool) -> Vec<Wall> {
    let t = |n: &str| if tahoe.is_empty() { String::new() } else { format!("{tahoe}/tahoe-{n}.jpg") };
    let mut w = vec![Wall { id: "time".into(), name: "Through the day".into(), preview: t("dawn"), path: String::new(), light: String::new(), dark: String::new() }];
    for (id, name) in [("day", "Day"), ("dusk", "Dusk"), ("night", "Night")] {
        w.push(Wall { id: id.into(), name: name.into(), preview: t(id), path: t(id), light: String::new(), dark: String::new() });
    }
    if !material.is_empty() {
        w.push(Wall {
            id: "material".into(),
            name: "Material, light and dark".into(),
            preview: format!("{material}/material-{}.jpg", if dark { "dark" } else { "light" }),
            path: String::new(),
            light: format!("{material}/material-light.jpg"),
            dark: format!("{material}/material-dark.jpg"),
        });
    }
    w
}

fn s<'a>(c: &'a Value, k: &str, d: &'a str) -> &'a str {
    c[k].as_str().unwrap_or(d)
}

/// The settings show this wallpaper now (Welcome.isWall)
pub fn is_wall(cfg: &Value, w: &Wall) -> bool {
    let c = &cfg["wallpaper"];
    match w.id.as_str() {
        "time" => s(c, "dynamic", "off") == "time" && s(c, "dynamicFolder", "").is_empty(),
        "material" => s(c, "dynamic", "off") == "appearance" && s(c, "lightPath", "") == w.light && s(c, "darkPath", "") == w.dark,
        _ => s(c, "dynamic", "off") == "off" && s(c, "live", "").is_empty() && (s(c, "path", "") == w.path || (w.id == "dusk" && s(c, "path", "").is_empty())),
    }
}

/// A picture of your own: anything the list doesn't have (Welcome.ownWall)
pub fn own_wall(cfg: &Value, walls: &[Wall]) -> bool {
    let c = &cfg["wallpaper"];
    !walls.iter().any(|w| is_wall(cfg, w)) && (s(c, "dynamic", "off") != "off" || !s(c, "path", "").is_empty() || !s(c, "live", "").is_empty())
}

/// The light / dark / auto choice the settings have
pub fn mode(cfg: &Value) -> String {
    cfg["appearance"]["mode"].as_str().unwrap_or("dark").to_string()
}

/// Motion off (Reduce motion): no travel or fades
pub fn still(cfg: &Value) -> bool {
    cfg["appearance"]["reduceMotion"].as_bool().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mock: what would have been asked of the shell
    #[derive(Default)]
    struct Recorder(Vec<(String, Value)>);
    impl Ask for Recorder {
        fn ask(&mut self, call: &'static str, args: Value) {
            self.0.push((call.to_string(), args));
        }
    }

    #[test]
    fn four_steps_then_start() {
        let mut a = Recorder::default();
        let mut f = Flow::new("glass", true);
        assert_eq!((f.step, f.title()), (0, "Welcome to Tessera"));
        f.back();
        assert_eq!(f.step, 0);
        f.next(&mut a);
        f.next(&mut a);
        f.next(&mut a);
        assert_eq!((f.step, f.title()), (3, "You're all set"));
        assert!(a.0.is_empty(), "nothing asked before the end");
        assert_eq!(f.help(), "Everything you picked can be changed in Settings.");
        f.back();
        f.pick_look("material");
        f.next(&mut a);
        assert_eq!(f.help(), "Tessera switches to Material when you start. Your apps keep running.");
        f.next(&mut a);
        assert!(f.done);
        assert_eq!(a.0, vec![("welcome.finish".to_string(), json!(["material"]))]);
        // (once only)
        f.next(&mut a);
        f.later(&mut a);
        assert_eq!(a.0.len(), 1);
    }

    #[test]
    fn later_puts_it_away_and_material_needs_its_files() {
        let mut a = Recorder::default();
        let mut f = Flow::new("material", false);
        assert_eq!(f.looks(), vec!["glass"]);
        f.pick_look("glass");
        f.pick_look("material");
        assert_eq!(f.look, "glass");
        f.step = 3;
        assert_eq!(f.help(), "Tessera switches to Glass when you start. Your apps keep running.");
        f.later(&mut a);
        assert_eq!(a.0, vec![("welcome.later".to_string(), json!([]))]);
    }

    #[test]
    fn choices_go_to_the_shell_never_the_system() {
        let mut a = Recorder::default();
        let f = Flow::new("glass", true);
        let w = walls("/s/tahoe", "/m", true);
        f.pick_mode(&mut a, "auto");
        f.pick_mode(&mut a, "sepia");
        f.pick_wall(&mut a, &w, "dusk");
        f.pick_wall(&mut a, &w, "../../etc/passwd");
        f.pick_wall(&mut a, &w, "own");
        assert_eq!(
            a.0,
            vec![("welcome.mode".to_string(), json!(["auto"])), ("welcome.wall".to_string(), json!(["dusk"])), ("welcome.pick".to_string(), json!([]))]
        );
    }

    #[test]
    fn the_wallpapers_and_which_is_chosen() {
        let w = walls("/s/tahoe", "/m", false);
        assert_eq!(w.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["time", "day", "dusk", "night", "material"]);
        assert_eq!(w[0].preview, "/s/tahoe/tahoe-dawn.jpg");
        assert_eq!(w[4].preview, "/m/material-light.jpg");
        assert_eq!(walls("", "", true).len(), 4);
        // defaults: the still dusk picture
        let cfg = json!({});
        assert!(is_wall(&cfg, &w[2]));
        assert!(!own_wall(&cfg, &w));
        let cfg = json!({"wallpaper": {"dynamic": "time", "dynamicFolder": ""}});
        assert!(is_wall(&cfg, &w[0]) && !is_wall(&cfg, &w[2]));
        let cfg = json!({"wallpaper": {"dynamic": "appearance", "lightPath": "/m/material-light.jpg", "darkPath": "/m/material-dark.jpg"}});
        assert!(is_wall(&cfg, &w[4]));
        let cfg = json!({"wallpaper": {"path": "/home/me/cat.png"}});
        assert!(own_wall(&cfg, &w));
        let cfg = json!({"wallpaper": {"path": "/s/tahoe/tahoe-night.jpg"}});
        assert!(is_wall(&cfg, &w[3]) && !own_wall(&cfg, &w));
        assert_eq!(mode(&json!({})), "dark");
        assert!(!still(&json!({})));
    }
}
