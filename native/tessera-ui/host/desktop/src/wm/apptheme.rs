//! AppTheme.qml: Match apps (Settings → Appearance): GTK, Qt, terminal and
//! browser colours follow Tessera's accent or the wallpaper, light or dark,
//! through matugen (assets/scripts/app-theme.sh), re-applied a moment after
//! any of those change; turning it off empties the generated colour files.
//! In the Material look the shell's own palette goes to the script as roles
//! (app-theme-render.py). The Material look also brings the rest of the
//! system along: window corners, border and shadow (Hyprland), the window
//! controls the glass plugin draws, and the pointer.

use super::hx;
use super::Io;
use serde_json::{json, Value};
use tessera_core::host::Host;
use tessera_core::theme::{self, Look};

#[derive(Default)]
pub struct AppTheme {
    applied: String,
    key: String,
    window_look: String,
    controls_style: String,
    cursor: String,
    last_radius: Value,
}

fn roles_map(look: &Look) -> std::collections::HashMap<String, String> {
    theme::material_roles(&look.seed, look.dark, &look.style).into_iter().collect()
}

fn hex6(c: &str) -> String {
    c.trim_start_matches('#').chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect()
}

/// matugen, or the shell's own palette in Material
fn available(look: &Look) -> bool {
    tessera_core::util::on_path("matugen") || look.material
}

fn source(cfg: &Value) -> String {
    cfg["appearance"]["appTheme"].as_str().unwrap_or("off").to_string()
}
fn targets(cfg: &Value) -> String {
    match cfg["appearance"]["appThemeTargets"].as_array() {
        Some(a) => a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(","),
        None => "gtk,qt,terminals,browsers".into(),
    }
}

/// The pointer theme for Settings → Appearance: Bibata or macOS; in the Material
/// look Bibata also when "Material pointer" is on
pub fn cursor_theme(cfg: &Value, material: bool) -> &'static str {
    if (material && cfg["appearance"]["materialCursor"].as_bool().unwrap_or(false)) || cfg["appearance"]["cursor"] == "bibata" {
        "Bibata-Modern-Classic"
    } else {
        "macOS"
    }
}

impl AppTheme {
    fn roles_json(&self, h: &Host) -> String {
        let look = &h.st.shell.look;
        if look.material && source(h.cfg()) != "off" {
            json!(roles_map(look)).to_string()
        } else {
            String::new()
        }
    }

    fn key(&self, h: &Host) -> String {
        let (cfg, look) = (h.cfg(), &h.st.shell.look);
        let src = source(cfg);
        [
            (!cfg.is_null()).to_string(),
            src.clone(),
            available(look).to_string(),
            look.dark.to_string(),
            if src != "off" { look.accent.clone() } else { String::new() },
            if src == "wallpaper" { cfg["wallpaper"]["path"].as_str().unwrap_or("").to_string() } else { String::new() },
            targets(cfg),
            self.roles_json(h),
        ]
        .join("|")
    }

    /// Settings changed: what Match apps, the Material look and the pointer follow
    pub fn config_changed(&mut self, h: &Host, io: &mut Io) {
        if h.cfg().is_null() {
            return;
        }
        let key = self.key(h);
        if key != self.key {
            self.key = key;
            io.arm("apptheme.settle", 1500); // after a slider or a wallpaper crossfade settles
        }
        let look = &h.st.shell.look;
        let roles = if look.material { roles_map(look) } else { Default::default() };
        let r = |n: &str| roles.get(n).cloned().unwrap_or_default();
        let wl = if look.material { [r("primary"), r("outlineVariant"), look.dark.to_string()].join("|") } else { String::new() };
        if wl != self.window_look {
            self.window_look = wl;
            io.arm("apptheme.windows", 250);
        }
        let radius = h.cfg()["appearance"]["windowRadius"].clone();
        if radius != self.last_radius {
            self.last_radius = radius;
            if look.material {
                io.arm("apptheme.windows", 250); // System re-applies the corner radius; this follows a moment later
            }
        }
        let cs = if look.material { format!("material {} {}", hex6(&r("surfaceContainerHigh")), hex6(&r("onSurfaceVariant"))) } else { String::new() };
        if cs != self.controls_style {
            self.controls_style = cs;
            self.send_controls_style(io);
        }
        let cf = if look.material { cursor_theme(h.cfg(), true).to_string() } else { String::new() };
        if cf != self.cursor {
            self.cursor = cf;
            if !self.cursor.is_empty() {
                io.arm("apptheme.cursor", 300);
            }
        }
    }

    pub fn tick_settle(&mut self, h: &Host, io: &mut Io) {
        let (cfg, look) = (h.cfg(), &h.st.shell.look);
        let key = self.key(h);
        if cfg.is_null() || key == self.applied {
            return;
        }
        let src = source(cfg);
        let avail = available(look);
        let Some(script) = hx::script("app-theme.sh") else { return };
        let mut argv = vec![
            script.to_string_lossy().into_owned(),
            if src == "off" || !avail { "off".into() } else if look.dark { "dark".into() } else { "light".into() },
            look.accent.chars().take(7).collect(),
            src.clone(),
            cfg["wallpaper"]["path"].as_str().unwrap_or("").to_string(),
            targets(cfg),
            if look.dark { "dark".into() } else { "light".into() },
        ];
        if look.material {
            argv.push(self.roles_json(h));
        }
        self.applied = key;
        let on = avail && src != "off";
        io.thread_then(
            move || std::process::Command::new(&argv[0]).args(&argv[1..]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false),
            move |_wm, _h, _io, ok| hx::state_set("appThemeOn", json!(ok && on)),
        );
    }

    /// Windows follow the Material look: round corners (a circular arc), a 1 px
    /// border, a faint tint of primary on the focused window (20%) and fainter
    /// outlineVariant (15%) on the rest, a lower, softer shadow and a lighter
    /// blur behind see-through panels. look.lua sets Material's corners at
    /// login and after a reload; this follows a moment later.
    pub fn tick_windows(&mut self, h: &Host) {
        let look = &h.st.shell.look;
        if !look.material || h.cfg().is_null() {
            return;
        }
        let roles = roles_map(look);
        let hex = |n: &str| hex6(roles.get(n).map(String::as_str).unwrap_or("000000"));
        // (CoolDown's lighter blur is the shell's state: not known here, so the blur is always set)
        hx::eval(format!(
            "hl.config({{ general = {{ col = {{ active_border = \"rgba({}33)\", inactive_border = \"rgba({}26)\" }} }}, decoration = {{ rounding_power = 2.0, shadow = {{ range = 18, render_power = 3, offset = {{ 0, 4 }}, color = \"rgba(000000{})\" }}, blur = {{ size = 4, passes = 2 }} }} }})",
            hex("primary"),
            hex("outlineVariant"),
            if look.dark { "59" } else { "2e" }
        ));
    }

    /// Window controls the glass plugin draws: round icon buttons on a tonal
    /// pill in Material (Glass keeps the plugin's traffic lights). A plugin
    /// built before this answers "unknown verb" and keeps its traffic lights.
    pub fn send_controls_style(&self, _io: &Io) {
        if self.controls_style.is_empty() {
            return;
        }
        let s = format!("controls-style {}", self.controls_style);
        std::thread::spawn(move || {
            hx::glass_command(&s);
        });
    }
    pub fn plugin_ready(&self, io: &Io) {
        self.send_controls_style(io);
    }

    /// The pointer: Material sets it again here, falling back to macOS when
    /// the theme isn't installed
    pub fn tick_cursor(&mut self) {
        if self.cursor.is_empty() {
            return;
        }
        hx::spawn(vec![
            "sh".into(),
            "-c".into(),
            "found() { for d in /usr/share/icons \"$HOME/.local/share/icons\" \"$HOME/.icons\"; do [ -d \"$d/$1\" ] && return 0; done; return 1; }; t=$1; found \"$t\" || t=macOS; h=$t; [ \"$t\" = macOS ] && found macOS-hypr && h=macOS-hypr; hyprctl setcursor \"$h\" 24 >/dev/null; gsettings set org.gnome.desktop.interface cursor-theme \"$t\" 2>/dev/null".into(),
            "sh".into(),
            self.cursor.clone(),
        ]);
    }

    pub fn event(&mut self, h: &Host, io: &mut Io, name: &str) {
        if name == "configreloaded" && h.st.shell.look.material {
            io.arm("apptheme.windows", 250);
        }
    }
}
