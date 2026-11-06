//! The look, from the shell's own config.json ($XDG_CONFIG_HOME/tessera):
//! appearance.look (glass | material), .mode (dark | light | auto) and
//! .materialSeed / .materialStyle. Glass colours follow shell/common/Theme.qml;
//! Material's come from the seed through material-colors (Material Color
//! Utilities in Rust, spec 2025 as shell/common/MaterialColor.js asks for).

use material_colors::color::Rgb;
use material_colors::dynamic_color::{DynamicScheme, Platform, Role, SpecVersion, Variant};
use material_colors::hct::Hct;
use serde_json::Value;
use slint::Color;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub material: bool,
    pub dark: bool,
    pub seed: String,
    pub style: String,
    /// Glass's accent, #rrggbb (Settings' swatch, or the wallpaper's)
    pub accent: String,
    /// 0 standard, 1 higher, 2 maximum (Theme.qml's contrastLevel)
    pub contrast: u8,
    /// Material: how see-through panels are, 0..1 (Theme.qml's transparency:
    /// appearance.materialTransparency, 0 with Reduce transparency; Glass: 0)
    pub transparency: f32,
    /// The glass material Settings chose (Theme.glass: Glass, Frost, Solid,
    /// Custom, the optics, the wallpaper's tint, Cool-down's cheaper glass)
    pub glass: GlassMaterial,
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    config_in(&base)
}

/// As shell/common/Paths.qml: Tessera's folder, or iRice's while only that one
/// has settings (not moved yet: the shell moves it at its next start)
pub fn config_in(base: &std::path::Path) -> PathBuf {
    let new = base.join("tessera/config.json");
    let old = base.join("irice/config.json");
    if !new.exists() && old.exists() {
        old
    } else {
        new
    }
}

/// The shell's settings, as JSON (Null when there are none yet)
pub fn read_json() -> Value {
    std::fs::read_to_string(config_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

pub fn read() -> Look {
    let mut v = read_json();
    // The seed and a wallpaper accent come from the picture showing now (the
    // bundled default when none is chosen, the dynamic one, a live poster), as
    // the shell's do, not only from the chosen file
    let material = crate::env::var("LOOK").as_deref().unwrap_or(v["appearance"]["look"].as_str().unwrap_or("glass")) == "material";
    let showing = crate::wallpaper::showing_now(&v, material);
    if v["wallpaper"].is_object() || v["wallpaper"].is_null() {
        v["wallpaper"]["path"] = Value::String(showing);
    }
    // TESSERA_LOOK=glass|material overrides (to measure both looks from one config)
    look_from(&v, crate::env::var("LOOK").as_deref(), wallpaper_seed)
}

/// The look a config says. `wallpaper`: the seed for a picture, when the
/// Material look follows the wallpaper (as the shell, Material's baseline
/// purple when there's none to read)
pub fn look_from(v: &Value, look_override: Option<&str>, wallpaper: impl Fn(&str) -> Option<String>) -> Look {
    look_with(v, look_override, wallpaper, &shell_glass())
}

/// The same, with what the shell said about the glass given
pub fn look_with(v: &Value, look_override: Option<&str>, wallpaper: impl Fn(&str) -> Option<String>, shell: &ShellGlass) -> Look {
    let a = &v["appearance"];
    let s = |k: &str, d: &str| a[k].as_str().unwrap_or(d).to_string();
    let look = look_override.map(str::to_string).unwrap_or_else(|| s("look", "glass"));
    let chosen = s("materialSeed", "wallpaper");
    let material = look == "material";
    let seed = if is_hex(&chosen) {
        chosen
    } else if material {
        v["wallpaper"]["path"].as_str().filter(|p| !p.is_empty()).and_then(&wallpaper).unwrap_or_else(|| BASELINE.into())
    } else {
        BASELINE.into()
    };
    let dark = s("mode", "dark") != "light"; // ("auto" follows the sun in the shell; dark here)
    let accent_name = s("accent", "blue");
    let accent = if accent_name == "wallpaper" {
        v["wallpaper"]["path"].as_str().filter(|p| !p.is_empty()).and_then(&wallpaper).map_or_else(|| accent_hex("blue", dark), |c| snap_accent(&c, dark))
    } else {
        accent_hex(&accent_name, dark)
    };
    let contrast = match s("contrast", "standard").as_str() {
        "maximum" => 2,
        "higher" => 1,
        _ if a["increaseContrast"].as_bool().unwrap_or(false) => 1,
        _ => 0,
    };
    let reduce = a["reduceTransparency"].as_bool().unwrap_or(false);
    // (Theme.transparency: none while Cool-down makes glass solid either)
    let transparency = if material && !reduce && shell.cooldown != "solid" { a["materialTransparency"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let style = s("materialStyle", "tonalSpot");
    let glass = glass_material(v, material, dark, transparency, shell, || {
        let roles = material_roles(&seed, dark, &style);
        rgbf(roles.iter().find(|(n, _)| n == "surfaceContainer").map_or("#000000", |(_, c)| c.as_str()))
    });
    Look { material, dark, seed, style, accent, contrast, transparency, glass }
}

// ── Glass: Theme.glass (shell/common/Theme.qml) ─────────────────────────────

/// What only the QML shell knows that the glass follows, as it last said
/// (RustUi.qml's {"src":"shell","kind":"glass"} line): the wallpaper's tone
/// (Wallpaper.tone, for appearance.wallpaperTint) and Cool-down's glass
/// (CoolDown.glass: keep, frosted or solid)
#[derive(Clone, Debug, PartialEq)]
pub struct ShellGlass {
    pub tone: Option<[f32; 3]>,
    pub cooldown: String,
}

impl Default for ShellGlass {
    fn default() -> Self {
        Self { tone: None, cooldown: "keep".into() }
    }
}

static SHELL_GLASS: std::sync::Mutex<Option<ShellGlass>> = std::sync::Mutex::new(None);

pub fn shell_glass() -> ShellGlass {
    SHELL_GLASS.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// The shell said it again; true when it changed (the look is read again)
pub fn set_shell_glass(g: ShellGlass) -> bool {
    let Ok(mut cur) = SHELL_GLASS.lock() else { return false };
    let changed = cur.as_ref() != Some(&g);
    *cur = Some(g);
    changed
}

/// The glass line from the shell: {"src":"shell","kind":"glass","tone":"#rrggbb","cooldown":"keep"}
pub fn shell_glass_from(v: &Value) -> ShellGlass {
    let tone = v["tone"].as_str().filter(|t| t.len() >= 7 && is_hex(&t[..7])).map(|t| {
        let c = rgbf(&t[..7]);
        [c[0], c[1], c[2]]
    });
    let cooldown = v["cooldown"].as_str().filter(|c| ["keep", "frosted", "solid"].contains(c)).unwrap_or("keep").to_string();
    ShellGlass { tone, cooldown }
}

/// "#rrggbb" as 0..1 channels, opaque
fn rgbf(s: &str) -> [f32; 4] {
    let v = u32::from_str_radix(s.trim_start_matches('#').get(..6).unwrap_or("000000"), 16).unwrap_or(0);
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0, 1.0]
}

/// A material: lens (bends what's behind), frost (blurs it), solid
/// (opacity) and tint (darkens it), with the optics and colours it's drawn
/// with (Theme.glass's properties, as they are once its glide settles)
#[derive(Clone, Debug, PartialEq)]
pub struct GlassMaterial {
    /// the material after Reduce transparency and Cool-down (liquid,
    /// frosted, solid, custom, veil)
    pub name: String,
    pub lens: f32,
    pub frost: f32,
    pub solid: f32,
    pub tint_amount: f32,
    /// thin | depth | auto (thick on small controls)
    pub depth: String,
    pub material_look: bool,
    pub dark: bool,
    /// the clear tint (Theme.glass.tint: the base, toward the wallpaper's tone)
    pub tint: [f32; 4],
    pub frost_color: [f32; 4],
    pub solid_color: [f32; 4],
    pub bend: f32,
    pub band: f32,
    pub dispersion: f32,
}

/// The plugin's numbers for one shape's material (Theme.glass.preset)
#[derive(Clone, Debug, PartialEq)]
pub struct GlassPreset {
    pub mode: f32,
    pub refraction: f32,
    pub depth: f32,
    pub bevel: f32,
    pub frost: f32,
    pub chroma: f32,
    pub rim: f32,
    pub gloss: f32,
    pub tint: [f32; 4],
    pub saturation: f32,
}

impl GlassMaterial {
    /// Theme.glass.gloss: 3D lighting, full for clear glass, none solid
    pub fn gloss(&self) -> f32 {
        self.lens * (1.0 - 0.85 * self.frost) * (1.0 - self.solid)
    }
    /// Theme.glass.tier: the nearest named material
    pub fn tier(&self) -> &'static str {
        if self.solid >= 0.97 {
            "solid"
        } else if self.solid < 0.4 && (self.frost < 0.4 || self.lens >= 0.6) {
            "liquid"
        } else {
            "frosted"
        }
    }
    /// Theme.glass.preset(minSide)
    pub fn preset(&self, min_side: f32) -> GlassPreset {
        let (l, f, s) = (self.lens, self.frost, self.solid);
        let thick = self.depth == "depth" || (self.depth == "auto" && min_side <= 140.0);
        if s >= 0.99 {
            return GlassPreset { mode: 2.0, refraction: 0.0, depth: 0.0, bevel: 8.0, frost: 0.0, chroma: 0.0, rim: 0.0, gloss: 0.0, tint: self.solid_color, saturation: 1.0 };
        }
        let ta = self.tint_amount;
        let k = 0.3 + 1.4 * ta;
        let alpha = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        let clear = alpha(self.tint, (if self.dark { 0.16 } else { 0.18 } * k).min(0.9));
        let frost = alpha(self.frost_color, (if self.dark { 0.38 } else { 0.45 } * (0.6 + 0.8 * ta)).min(0.9));
        let body = mixc(clear, frost, f);
        let t = mixc(body, alpha(self.solid_color, 1.0), ((s - body[3]) / (1.0 - body[3]).max(0.01)).max(0.0));
        GlassPreset {
            mode: 0.0,
            refraction: if thick { 30.0 } else { 22.0 } * l * self.bend,
            depth: if thick { 0.55 } else { 0.0 } * l * self.bend.min(1.5),
            bevel: if thick { 26.0 } else { 18.0 } * (0.5 + 0.5 * l) * self.band,
            frost: f,
            chroma: if thick { 0.28 } else { 0.18 } * l * self.dispersion,
            rim: if self.material_look { 0.0 } else { (0.12 + 0.16 * l) * (1.0 - s) },
            gloss: self.gloss(),
            tint: t,
            saturation: 1.35 + 0.15 * f - 0.5 * s,
        }
    }
}

/// Theme.glass.mixc: a toward b by t, alpha too
fn mixc(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

/// Theme.glass from the settings (appearance.glass, glassLens, glassFrost,
/// glassSolid, glassTint, glassIor, glassRefraction, glassDispersion,
/// glassDepth, wallpaperTint, reduceTransparency), the look, and what the
/// shell said (the wallpaper's tone, Cool-down). `m3_container`: Material's
/// surfaceContainer (asked only in that look)
pub fn glass_material(v: &Value, material: bool, dark: bool, transparency: f32, shell: &ShellGlass, m3_container: impl Fn() -> [f32; 4]) -> GlassMaterial {
    let a = &v["appearance"];
    let num = |k: &str, d: f64| a[k].as_f64().unwrap_or(d) as f32;
    let reduce = a["reduceTransparency"].as_bool().unwrap_or(false);
    let blur = transparency > 0.0;
    let panel_alpha = 1.0 - 0.55 * transparency;
    let control_alpha = 1.0 - 0.4 * transparency;
    let setting = a["glass"].as_str().unwrap_or("frosted");
    let chosen = if reduce {
        "solid"
    } else if material {
        if blur {
            "veil"
        } else {
            "solid"
        }
    } else if setting == "auto" {
        "frosted"
    } else {
        setting
    };
    // Cool-down swaps in a cheaper material (never a heavier one)
    let name = if shell.cooldown == "solid" {
        "solid"
    } else if shell.cooldown == "frosted" && chosen != "solid" && chosen != "veil" {
        "frosted"
    } else {
        chosen
    };
    let [lens, frost, solid, tint_amount] = match name {
        "custom" => [num("glassLens", 1.0), num("glassFrost", 0.75), num("glassSolid", 0.0), num("glassTint", 0.75)],
        "frosted" => [0.3, 1.0, 0.45, 0.5],
        "solid" => [0.0, 0.0, 1.0, 0.0],
        "veil" => [0.0, 1.0, panel_alpha, 0.5],
        _ => [1.0, 0.75, 0.0, 0.75],
    };
    let base = if dark { [0.08, 0.08, 0.1, 0.16] } else { [0.98, 0.98, 1.0, 0.3] };
    let tint = match (a["wallpaperTint"].as_bool().unwrap_or(true), shell.tone) {
        // (Qt.tint: the tone at 10% (light: 8%) over the base)
        (true, Some(t)) => {
            let ta = if dark { 0.1 } else { 0.08 };
            [t[0] * ta + base[0] * (1.0 - ta), t[1] * ta + base[1] * (1.0 - ta), t[2] * ta + base[2] * (1.0 - ta), ta + (1.0 - ta) * base[3]]
        }
        _ => base,
    };
    let frost_color = if dark { [0.11, 0.11, 0.13, 0.62] } else { [0.97, 0.97, 0.98, 0.66] };
    let solid_color = if material {
        let c = m3_container();
        [c[0], c[1], c[2], if transparency > 0.0 { control_alpha } else { 1.0 }]
    } else {
        rgbf(if dark { "#26262A" } else { "#F2F2F5" })
    };
    let ior = num("glassIor", 1.5).clamp(1.0, 2.0);
    GlassMaterial {
        name: name.to_string(),
        lens,
        frost,
        solid,
        tint_amount,
        depth: a["glassDepth"].as_str().unwrap_or("auto").to_string(),
        material_look: material,
        dark,
        tint,
        frost_color,
        solid_color,
        bend: (1.0 - 1.0 / ior) / (1.0 - 1.0 / 1.5),
        band: num("glassRefraction", 1.0).clamp(0.4, 2.0),
        dispersion: num("glassDispersion", 0.35).clamp(0.0, 1.0) / 0.35,
    }
}

/// Theme.qml's accents, by name (blue when unknown)
pub fn accent_hex(name: &str, dark: bool) -> String {
    let (d, l) = match name {
        "purple" => ("#BF5AF2", "#AF52DE"),
        "pink" => ("#FF375F", "#FF2D55"),
        "red" => ("#FF453A", "#FF3B30"),
        "orange" => ("#FF9F0A", "#FF9500"),
        "yellow" => ("#FFD60A", "#FFCC00"),
        "green" => ("#30D158", "#34C759"),
        "teal" => ("#40C8E0", "#30B0C7"),
        "cyan" => ("#64D2FF", "#32ADE6"),
        "indigo" => ("#5E5CE6", "#5856D6"),
        "graphite" => ("#98989D", "#8E8E93"),
        _ => ("#0A84FF", "#007AFF"),
    };
    (if dark { d } else { l }).to_string()
}

/// The wallpaper's colour snapped to the nearest named accent by hue
/// (graphite when it has hardly any colour), as Wallpaper.qml's snapAccent
pub fn snap_accent(color: &str, dark: bool) -> String {
    let rgb = Rgb::from_str(color).unwrap_or(Rgb::new(0x0a, 0x84, 0xff));
    let hct: Hct = rgb.into();
    if hct.get_chroma() < 12.0 {
        return accent_hex("graphite", dark);
    }
    let names = ["blue", "purple", "pink", "red", "orange", "yellow", "green", "teal", "cyan", "indigo"];
    let hue = |c: &str| -> f64 { let h: Hct = Rgb::from_str(c).unwrap().into(); h.get_hue() };
    let dist = |a: f64, b: f64| -> f64 { let d = (a - b).abs() % 360.0; d.min(360.0 - d) };
    let best = names.iter().min_by(|a, b| dist(hue(&accent_hex(a, dark)), hct.get_hue()).total_cmp(&dist(hue(&accent_hex(b, dark)), hct.get_hue()))).unwrap();
    accent_hex(best, dark)
}

const BASELINE: &str = "#6750A4";

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// The wallpaper's seed (src/seed.rs), remembered per picture: the config is
/// read again on every change, the picture rarely changes
fn wallpaper_seed(path: &str) -> Option<String> {
    use std::sync::Mutex;
    static LAST: Mutex<Option<(String, Option<String>)>> = Mutex::new(None);
    let mut last = LAST.lock().ok()?;
    if let Some((p, seed)) = last.as_ref() {
        if p == path {
            return seed.clone();
        }
    }
    let seed = crate::seed::from_picture(std::path::Path::new(path));
    *last = Some((path.to_string(), seed.clone()));
    seed
}

fn variant(style: &str) -> Variant {
    match style {
        "expressive" => Variant::Expressive,
        "vibrant" => Variant::Vibrant,
        "content" => Variant::Content,
        "neutral" => Variant::Neutral,
        "fidelity" => Variant::Fidelity,
        "monochrome" => Variant::Monochrome,
        _ => Variant::TonalSpot,
    }
}

/// Every Material role for a seed, as (camelCase name, #rrggbb)
pub fn material_roles(seed: &str, dark: bool, style: &str) -> Vec<(String, String)> {
    let rgb = Rgb::from_str(seed).unwrap_or(Rgb::new(0x67, 0x50, 0xA4));
    let hct: Hct = rgb.into();
    let scheme = DynamicScheme::from_spec(hct, variant(style), dark, Some(0.0), Platform::Phone, SpecVersion::Spec2025);
    Role::ALL
        .iter()
        .map(|r| {
            let c = scheme.get_rgb(r.color());
            let mut name = String::new();
            let mut up = false;
            for ch in r.name().chars() {
                if ch == '_' {
                    up = true;
                } else if up {
                    name.extend(ch.to_uppercase());
                    up = false;
                } else {
                    name.push(ch);
                }
            }
            (name, format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue))
        })
        .collect()
}

fn hex(s: &str) -> Color {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_argb_u8((a * 255.0).round() as u8, r, g, b)
}

/// The Theme global's tokens Rust sets (ui/theme.slint's `Theme`, after
/// shell/common/Theme.qml), each as `field / setter: type`: the one list.
/// Every surface crate compiles its own Slint, so each has its own generated
/// `Theme` type; `theme_global!` gives each the same `ThemeGlobal` impl from
/// this list. A new token: its property in ui/theme.slint, its line here, its
/// value in `tokens` (a token missing from either is a compile error).
#[doc(hidden)]
#[macro_export]
macro_rules! __theme_tokens {
    ($then:ident ! $args:tt) => {
        $crate::$then! {
            $args
            material / set_material: bool,
            dark / set_dark: bool,
            font / set_font: slint::SharedString,
            body / set_body: f32,
            bar_tint / set_bar_tint: slint::Color,
            fg / set_fg: slint::Color,
            pill / set_pill: slint::Color,
            dot / set_dot: slint::Color,
            dot_active / set_dot_active: slint::Color,
            menu_bg / set_menu_bg: slint::Color,
            menu_fg / set_menu_fg: slint::Color,
            menu_dim / set_menu_dim: slint::Color,
            menu_hover / set_menu_hover: slint::Color,
            menu_hover_fg / set_menu_hover_fg: slint::Color,
            separator / set_separator: slint::Color,
            field_bg / set_field_bg: slint::Color,
            field_focus / set_field_focus: slint::Color,
            menu_radius / set_menu_radius: f32,
            item_radius / set_item_radius: f32,
            label / set_label: slint::Color,
            secondary / set_secondary: slint::Color,
            tertiary / set_tertiary: slint::Color,
            outline / set_outline: slint::Color,
            fill / set_fill: slint::Color,
            fill_hover / set_fill_hover: slint::Color,
            fill_pressed / set_fill_pressed: slint::Color,
            platter / set_platter: slint::Color,
            toggle_off / set_toggle_off: slint::Color,
            accent / set_accent: slint::Color,
            on_accent / set_on_accent: slint::Color,
            highlight / set_highlight: slint::Color,
            on_highlight / set_on_highlight: slint::Color,
            selection / set_selection: slint::Color,
            on_selection / set_on_selection: slint::Color,
            danger / set_danger: slint::Color,
            success / set_success: slint::Color,
            warning / set_warning: slint::Color,
            panel / set_panel: slint::Color,
            m3_primary / set_m3_primary: slint::Color,
            m3_on_primary / set_m3_on_primary: slint::Color,
            m3_primary_container / set_m3_primary_container: slint::Color,
            m3_on_primary_container / set_m3_on_primary_container: slint::Color,
            m3_secondary / set_m3_secondary: slint::Color,
            m3_secondary_container / set_m3_secondary_container: slint::Color,
            m3_on_secondary_container / set_m3_on_secondary_container: slint::Color,
            m3_tertiary / set_m3_tertiary: slint::Color,
            m3_on_tertiary / set_m3_on_tertiary: slint::Color,
            m3_surface / set_m3_surface: slint::Color,
            m3_surface_container_low / set_m3_surface_container_low: slint::Color,
            m3_surface_container / set_m3_surface_container: slint::Color,
            m3_surface_container_high / set_m3_surface_container_high: slint::Color,
            m3_surface_container_highest / set_m3_surface_container_highest: slint::Color,
            m3_on_surface / set_m3_on_surface: slint::Color,
            m3_on_surface_variant / set_m3_on_surface_variant: slint::Color,
            m3_outline / set_m3_outline: slint::Color,
            m3_outline_variant / set_m3_outline_variant: slint::Color,
            m3_error / set_m3_error: slint::Color,
            m3_on_error / set_m3_on_error: slint::Color,
            m3_error_container / set_m3_error_container: slint::Color,
            m3_on_error_container / set_m3_on_error_container: slint::Color,
            m3_tertiary_container / set_m3_tertiary_container: slint::Color,
            m3_on_tertiary_container / set_m3_on_tertiary_container: slint::Color,
            m3_inverse_surface / set_m3_inverse_surface: slint::Color,
            m3_inverse_on_surface / set_m3_inverse_on_surface: slint::Color,
            blue / set_blue: slint::Color,
            yellow / set_yellow: slint::Color,
            radius_item / set_radius_item: f32,
            radius_tile / set_radius_tile: f32,
            m3_primary_fixed / set_m3_primary_fixed: slint::Color,
            m3_primary_fixed_dim / set_m3_primary_fixed_dim: slint::Color,
            m3_shadow / set_m3_shadow: slint::Color,
            m3_scrim / set_m3_scrim: slint::Color,
            glass_solid / set_glass_solid: slint::Color,
            blur / set_blur: bool,
            panel_alpha / set_panel_alpha: f32,
            m3_inverse_primary / set_m3_inverse_primary: slint::Color,
            accent_ink / set_accent_ink: slint::Color,
            accent_tint / set_accent_tint: slint::Color,
            code_fill / set_code_fill: slint::Color,
            control_alpha / set_control_alpha: f32,
            caption / set_caption: f32,
            small / set_small: f32,
            title / set_title: f32,
            large / set_large: f32,
            display / set_display: f32,
            radius_control / set_radius_control: f32,
            radius_popover / set_radius_popover: f32,
            radius_module / set_radius_module: f32,
            radius_panel / set_radius_panel: f32,
            spring / set_spring: f32,
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __theme_struct {
    (() $($f:ident / $set:ident : $ty:ty,)*) => {
        /// What `tokens` gives for a look; None: not set for this look
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Tokens {
            $(pub $f: Option<$ty>,)*
        }
        impl Tokens {
            $(pub fn $set(&mut self, v: $ty) {
                self.$f = Some(v);
            })*
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __theme_set {
    (($t:ident $k:ident) $($f:ident / $set:ident : $ty:ty,)*) => {
        $(if let Some(v) = &$k.$f {
            $t.$set(v.clone());
        })*
    };
}

crate::__theme_tokens!(__theme_struct!());

/// A crate's generated Slint `Theme` global, filled from `Tokens`
/// (implemented by `theme_global!`)
pub trait ThemeGlobal {
    fn apply_tokens(&self, k: &Tokens);
}

/// In a surface crate's lib.rs, after its `ui` module:
/// `tessera_core::theme_global!(ui);` Then `st.shell.apply_theme(&ui.global::<ui::Theme>())`
/// fills that crate's Theme for the current look.
#[macro_export]
macro_rules! theme_global {
    ($ui:ident) => {
        impl<'a> $crate::theme::ThemeGlobal for $ui::Theme<'a> {
            fn apply_tokens(&self, k: &$crate::theme::Tokens) {
                let t = self;
                $crate::__theme_tokens!(__theme_set!(t k));
            }
        }
    };
}

/// Fill a Slint `Theme` global for a look (any crate's, through `ThemeGlobal`)
pub fn apply(t: &impl ThemeGlobal, look: &Look, plugin: bool) {
    t.apply_tokens(&tokens(look, plugin));
}

/// The Theme global's values for a look. `plugin`: the glass plugin draws
/// the glass under the bar (otherwise Hyprland's blur needs more alpha).
/// (A token a look doesn't set stays as it was: Glass leaves the m3-* ones.)
pub fn tokens(look: &Look, plugin: bool) -> Tokens {
    let mut tokens = Tokens::default();
    let t = &mut tokens;
    t.set_material(look.material);
    t.set_dark(look.dark);
    if look.material {
        let roles = material_roles(&look.seed, look.dark, &look.style);
        let r = |k: &str| hex(roles.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("#ff00ff"));
        t.set_font("Tessera Sans Flex".into());
        t.set_body(14.0);
        t.set_bar_tint(Color::from_argb_u8(0, 0, 0, 0));
        t.set_fg(r("onSurface"));
        // (MenuBar.qml's pillColor: the island's own surface, surfaceContainerHigh)
        // (veiled: see-through with Material's transparency, Theme.veil)
        t.set_pill(if look.transparency > 0.0 { r("surfaceContainerHigh").with_alpha(1.0 - 0.55 * look.transparency) } else { r("surfaceContainerHigh") });
        t.set_dot(r("onSurfaceVariant"));
        t.set_dot_active(r("primary"));
        t.set_menu_bg(r("surfaceContainer"));
        t.set_menu_fg(r("onSurface"));
        t.set_menu_dim(r("onSurfaceVariant"));
        t.set_menu_hover(r("onSurface").with_alpha(0.1));
        t.set_menu_hover_fg(r("onSurface"));
        t.set_separator(r("outlineVariant"));
        t.set_field_bg(r("surfaceContainerHighest"));
        t.set_field_focus(r("primary"));
        t.set_menu_radius(16.0);
        t.set_item_radius(12.0);
        let l = |s: f32, h: f32, m: f32| [s, h, m][look.contrast.min(2) as usize];
        let on = r("onSurface");
        t.set_label(on);
        t.set_secondary(r("onSurfaceVariant"));
        t.set_tertiary(r("onSurfaceVariant").with_alpha(l(0.72, 0.85, 1.0)));
        t.set_separator(r("outlineVariant").with_alpha(l(0.45, 0.7, 1.0)));
        t.set_outline(r("outline"));
        let hi = r("surfaceContainerHighest");
        t.set_fill(hi);
        t.set_fill_hover(tint(hi, on, 0.08));
        t.set_fill_pressed(tint(hi, on, 0.10));
        t.set_platter(r("surfaceContainerHigh"));
        t.set_toggle_off(hi);
        t.set_accent(r("primary"));
        t.set_on_accent(r("onPrimary"));
        t.set_highlight(on.with_alpha(0.1));
        t.set_on_highlight(on);
        t.set_selection(r("secondaryContainer"));
        t.set_on_selection(r("onSecondaryContainer"));
        t.set_danger(r("error"));
        t.set_success(hex(if look.dark { "#30D158" } else { "#34C759" }));
        t.set_warning(hex(if look.dark { "#FF9F0A" } else { "#FF9500" }));
        t.set_panel(r("surfaceContainer"));
        t.set_m3_primary(r("primary"));
        t.set_m3_on_primary(r("onPrimary"));
        t.set_m3_primary_container(r("primaryContainer"));
        t.set_m3_on_primary_container(r("onPrimaryContainer"));
        t.set_m3_secondary(r("secondary"));
        t.set_m3_secondary_container(r("secondaryContainer"));
        t.set_m3_on_secondary_container(r("onSecondaryContainer"));
        t.set_m3_tertiary(r("tertiary"));
        t.set_m3_on_tertiary(r("onTertiary"));
        t.set_m3_surface(r("surface"));
        t.set_m3_surface_container_low(r("surfaceContainerLow"));
        t.set_m3_surface_container(r("surfaceContainer"));
        t.set_m3_surface_container_high(r("surfaceContainerHigh"));
        t.set_m3_surface_container_highest(hi);
        t.set_m3_on_surface(on);
        t.set_m3_on_surface_variant(r("onSurfaceVariant"));
        t.set_m3_outline(r("outline"));
        t.set_m3_outline_variant(r("outlineVariant"));
        t.set_m3_error(r("error"));
        t.set_m3_on_error(r("onError"));
        t.set_m3_error_container(r("errorContainer"));
        t.set_m3_on_error_container(r("onErrorContainer"));
        t.set_m3_tertiary_container(r("tertiaryContainer"));
        t.set_m3_on_tertiary_container(r("onTertiaryContainer"));
        t.set_m3_inverse_surface(r("inverseSurface"));
        t.set_m3_inverse_on_surface(r("inverseOnSurface"));
        t.set_blue(hex(if look.dark { "#0A84FF" } else { "#007AFF" }));
        t.set_yellow(hex(if look.dark { "#FFD60A" } else { "#FFCC00" }));
        t.set_radius_item(8.0);
        t.set_radius_tile(28.0);
        t.set_m3_primary_fixed(r("primaryFixed"));
        t.set_m3_primary_fixed_dim(r("primaryFixedDim"));
        t.set_m3_shadow(r("shadow"));
        t.set_m3_scrim(r("scrim"));
        t.set_glass_solid(r("surfaceContainer"));
        t.set_blur(look.transparency > 0.0);
        t.set_panel_alpha(1.0 - 0.55 * look.transparency);
        t.set_m3_inverse_primary(r("inversePrimary"));
        t.set_accent_ink(r("primary"));
        t.set_accent_tint(r("secondaryContainer"));
        t.set_code_fill(hi);

        t.set_control_alpha(1.0 - 0.4 * look.transparency);
        t.set_caption(11.0);
        t.set_small(12.0);
        t.set_title(16.0);
        t.set_large(22.0);
        t.set_display(28.0);
        t.set_radius_control(12.0);
        t.set_radius_popover(28.0);
        t.set_radius_module(24.0);
        t.set_radius_panel(28.0);
        t.set_spring(0.5);
    } else {
        let d = look.dark;
        t.set_font("Inter".into());
        t.set_body(13.0);
        // Over the plugin's glass the strip is nearly clear; Hyprland's own
        // blur (the fallback) only applies above 40% alpha (ignore_alpha)
        t.set_bar_tint(if d { rgba(0x26, 0x26, 0x2a, if plugin { 0.12 } else { 0.6 }) } else { rgba(0xf2, 0xf2, 0xf5, if plugin { 0.12 } else { 0.6 }) });
        let fg = if d { rgba(255, 255, 255, 0.92) } else { rgba(0, 0, 0, 0.85) };
        t.set_fg(fg);
        t.set_pill(Color::from_argb_u8(0, 0, 0, 0));
        t.set_dot(if d { rgba(255, 255, 255, 1.0) } else { rgba(0, 0, 0, 1.0) });
        t.set_dot_active(fg.with_alpha(0.28));
        t.set_menu_bg(if d { rgba(0x1e, 0x1e, 0x22, if plugin { 0.45 } else { 0.85 }) } else { rgba(0xf6, 0xf6, 0xf8, if plugin { 0.5 } else { 0.88 }) });
        t.set_menu_fg(fg);
        t.set_menu_dim(fg.with_alpha(0.55));
        t.set_field_bg(fg.with_alpha(0.08));
        t.set_menu_radius(12.0);
        t.set_item_radius(6.0);
        let l = |s: f32, h: f32, m: f32| [s, h, m][look.contrast.min(2) as usize];
        let ink = |a: f32| if d { rgba(255, 255, 255, a) } else { rgba(0, 0, 0, a) };
        t.set_label(ink(if d { l(0.92, 1.0, 1.0) } else { l(0.85, 0.9, 0.95) }));
        t.set_secondary(ink(if d { l(0.58, 0.72, 0.85) } else { l(0.52, 0.68, 0.8) }));
        t.set_tertiary(ink(if d { l(0.34, 0.56, 0.7) } else { l(0.3, 0.6, 0.7) }));
        t.set_separator(ink(if d { l(0.12, 0.14, 0.16) } else { l(0.1, 0.12, 0.14) }));
        t.set_outline(ink(if d { 0.42 } else { 0.38 }));
        t.set_fill(ink(if d { l(0.1, 0.14, 0.18) } else { l(0.06, 0.09, 0.12) }));
        t.set_fill_hover(ink(if d { l(0.16, 0.2, 0.24) } else { l(0.1, 0.13, 0.16) }));
        t.set_fill_pressed(ink(if d { l(0.22, 0.26, 0.3) } else { l(0.15, 0.18, 0.21) }));
        t.set_platter(if d { rgba(255, 255, 255, 0.07) } else { rgba(255, 255, 255, 0.45) });
        t.set_toggle_off(ink(if d { l(0.14, 0.24, 0.32) } else { l(0.08, 0.16, 0.24) }));
        let accent = hex(&look.accent);
        // (near-black on light accents, as Theme.qml's lightAccent: white on them is under 3:1)
        let on_accent = if contrast_ratio(accent, Color::from_rgb_u8(255, 255, 255)) < 3.0 { hex("#1C1C1E") } else { Color::from_rgb_u8(255, 255, 255) };
        t.set_accent(accent);
        t.set_on_accent(on_accent);
        t.set_highlight(accent);
        t.set_on_highlight(on_accent);
        t.set_selection(accent);
        t.set_on_selection(on_accent);
        t.set_menu_hover(accent);
        t.set_menu_hover_fg(on_accent);
        t.set_accent_ink(accent_ink(accent, if d { hex("#1E1E21") } else { hex("#F5F5F7") }));
        // (Theme.qml's pale ramp shade in light mode: the accent near white)
        t.set_accent_tint(if d { accent.with_alpha(0.26) } else { mix(accent, Color::from_rgb_u8(255, 255, 255), 0.82).with_alpha(0.9) });
        t.set_code_fill(if d { rgba(255, 255, 255, 0.08) } else { rgba(0, 0, 0, 0.05) });
        t.set_field_focus(accent);
        t.set_danger(hex(if d { "#FF453A" } else { "#FF3B30" }));
        t.set_success(hex(if d { "#30D158" } else { "#34C759" }));
        t.set_warning(hex(if d { "#FF9F0A" } else { "#FF9500" }));
        t.set_panel(if d { rgba(0x1e, 0x1e, 0x22, if plugin { 0.45 } else { 0.85 }) } else { rgba(0xf6, 0xf6, 0xf8, if plugin { 0.5 } else { 0.88 }) });
        t.set_blue(hex(if d { "#0A84FF" } else { "#007AFF" }));
        t.set_yellow(hex(if d { "#FFD60A" } else { "#FFCC00" }));
        t.set_radius_item(6.0);
        // (iOS tiles: half a Control Center button, Theme.radius.tile)
        t.set_radius_tile(36.0);
        t.set_glass_solid(hex(if d { "#26262A" } else { "#F2F2F5" }));
        t.set_blur(false);
        t.set_panel_alpha(1.0);
        t.set_control_alpha(1.0);
        t.set_caption(11.0);
        t.set_small(12.0);
        t.set_title(15.0);
        t.set_large(20.0);
        t.set_display(24.0);
        t.set_radius_control(10.0);
        t.set_radius_popover(18.0);
        t.set_radius_module(20.0);
        t.set_radius_panel(26.0);
        t.set_spring(0.32);
    }
    tokens
}

/// `a` toward `b` by `t`
fn mix(a: Color, b: Color, t: f32) -> Color {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::from_rgb_u8(m(a.red(), b.red()), m(a.green(), b.green()), m(a.blue(), b.blue()))
}

/// Accent text on a background, at least 4.5:1 (Theme.accentFor): the accent
/// when it reads; on a dark background a lighter shade of it (here: toward
/// white, in place of AccentRamp's OKLCH ramp), else near-black
fn accent_ink(accent: Color, bg: Color) -> Color {
    if contrast_ratio(accent, bg) >= 4.5 {
        return accent;
    }
    let dark_bg = contrast_ratio(bg, Color::from_rgb_u8(0, 0, 0)) < contrast_ratio(bg, Color::from_rgb_u8(255, 255, 255));
    if !dark_bg {
        return hex("#1C1C1E");
    }
    (1..=20).map(|i| mix(accent, Color::from_rgb_u8(255, 255, 255), i as f32 * 0.05)).find(|c| contrast_ratio(*c, bg) >= 4.5).unwrap_or(Color::from_rgb_u8(255, 255, 255))
}

/// `over` laid on `base` at `alpha` (Qt.tint)
fn tint(base: Color, over: Color, alpha: f32) -> Color {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * alpha).round() as u8;
    Color::from_argb_u8(base.alpha(), m(base.red(), over.red()), m(base.green(), over.green()), m(base.blue(), over.blue()))
}

/// WCAG contrast between two opaque colours
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let lum = |c: Color| {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.red()) + 0.7152 * ch(c.green()) + 0.0722 * ch(c.blue())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The plugin's numbers for one glass shape, as the QML shell's Glass.qml
/// sends them (Theme.glass.preset for the material Settings chose): x y w h
/// radius mode refraction depth bevel frost chroma rim tintRGBA saturation
/// opacity gloss. (`lens` < 1: menus under the bar bend less, MaterialMenu's
/// `lens: 0.35`.) The solid tier, which the QML draws itself without the
/// plugin, is the plugin's solid shape here (mode 2, the solid colour).
/// (Theme.qml glides 0.6 s between materials; here a change shows at once.)
pub fn glass_shape(look: &Look, x: f32, y: f32, w: f32, h: f32, radius: f32, opacity: f32, lens: f32) -> [f32; 19] {
    let g = &look.glass;
    let p = if g.tier() == "solid" { GlassMaterial { solid: 1.0, ..g.clone() }.preset(w.min(h)) } else { g.preset(w.min(h)) };
    [
        x, y, w, h, radius, p.mode,
        p.refraction * lens,
        p.depth * lens,
        p.bevel.min(radius),
        p.frost,
        p.chroma * lens,
        p.rim,
        p.tint[0], p.tint[1], p.tint[2], p.tint[3],
        p.saturation,
        opacity,
        p.gloss,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn accents_follow_settings() {
        let v = json!({"appearance": {"accent": "orange", "mode": "light"}});
        assert_eq!(look_from(&v, None, none).accent, "#FF9500");
        assert_eq!(look_from(&json!({}), None, none).accent, "#0A84FF");
        // the wallpaper's colour, snapped: a sea blue is blue, a leaf green green, grey graphite
        assert_eq!(snap_accent("#1e6fd9", true), "#0A84FF");
        assert_eq!(snap_accent("#3f8f2a", true), "#30D158");
        assert_eq!(snap_accent("#808080", true), "#98989D");
        let v = json!({"appearance": {"accent": "wallpaper"}, "wallpaper": {"path": "/p.jpg"}});
        assert_eq!(look_from(&v, None, |_| Some("#3f8f2a".into())).accent, "#30D158");
    }

    #[test]
    fn contrast_levels() {
        assert_eq!(look_from(&json!({}), None, none).contrast, 0);
        assert_eq!(look_from(&json!({"appearance": {"increaseContrast": true}}), None, none).contrast, 1);
        assert_eq!(look_from(&json!({"appearance": {"contrast": "maximum"}}), None, none).contrast, 2);
    }

    #[test]
    fn light_accents_get_dark_text() {
        assert!(contrast_ratio(hex("#FFD60A"), Color::from_rgb_u8(255, 255, 255)) < 3.0);
        assert!(contrast_ratio(hex("#0A84FF"), Color::from_rgb_u8(255, 255, 255)) >= 3.0);
    }

    #[test]
    fn material_panels_see_through_as_settings_say() {
        assert_eq!(look_from(&json!({"appearance": {"look": "material"}}), None, none).transparency, 0.0);
        let v = json!({"appearance": {"look": "material", "materialTransparency": 0.6}});
        assert!((look_from(&v, None, none).transparency - 0.6).abs() < 1e-6);
        let v = json!({"appearance": {"look": "material", "materialTransparency": 0.6, "reduceTransparency": true}});
        assert_eq!(look_from(&v, None, none).transparency, 0.0);
        let v = json!({"appearance": {"look": "glass", "materialTransparency": 0.6}});
        assert_eq!(look_from(&v, None, none).transparency, 0.0);
    }

    #[test]
    fn glass_by_default() {
        let l = look_from(&Value::Null, None, none);
        assert!(!l.material && l.dark);
        assert_eq!(l.style, "tonalSpot");
    }

    #[test]
    fn material_with_a_chosen_seed() {
        let v = json!({"appearance": {"look": "material", "mode": "light", "materialSeed": "#3b6ea5", "materialStyle": "expressive"}});
        let l = look_from(&v, None, |_| panic!("the wallpaper isn't read when a seed is chosen"));
        assert_eq!((l.material, l.dark, l.seed.as_str(), l.style.as_str()), (true, false, "#3b6ea5", "expressive"));
    }

    #[test]
    fn material_follows_the_wallpaper() {
        let v = json!({"appearance": {"look": "material", "materialSeed": "wallpaper"}, "wallpaper": {"path": "/pictures/sea.jpg"}});
        assert_eq!(look_from(&v, None, |p| (p == "/pictures/sea.jpg").then(|| "#1e6a8c".into())).seed, "#1e6a8c");
        // (an unreadable picture, or none set: the baseline)
        assert_eq!(look_from(&v, None, none).seed, BASELINE);
        let v = json!({"appearance": {"look": "material"}, "wallpaper": {"path": ""}});
        assert_eq!(look_from(&v, None, |_| panic!("no picture to read")).seed, BASELINE);
    }

    #[test]
    fn glass_never_reads_the_wallpaper() {
        let v = json!({"appearance": {"look": "glass"}, "wallpaper": {"path": "/pictures/sea.jpg"}});
        look_from(&v, None, |_| panic!("Glass has no seed"));
    }

    #[test]
    fn the_override_picks_the_look() {
        let v = json!({"appearance": {"look": "glass", "materialSeed": "#112233"}});
        assert!(look_from(&v, Some("material"), none).material);
    }

    #[test]
    fn a_bad_seed_is_treated_as_the_wallpaper() {
        let v = json!({"appearance": {"look": "material", "materialSeed": "#12345z"}});
        assert_eq!(look_from(&v, None, none).seed, BASELINE);
    }

    #[test]
    fn config_folder_follows_the_rename() {
        let d = std::env::temp_dir().join(format!("tessera-ui-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("irice")).unwrap();
        std::fs::write(d.join("irice/config.json"), "{}").unwrap();
        // only iRice's: that one, until the shell moves it
        assert_eq!(config_in(&d), d.join("irice/config.json"));
        std::fs::create_dir_all(d.join("tessera")).unwrap();
        std::fs::write(d.join("tessera/config.json"), "{}").unwrap();
        assert_eq!(config_in(&d), d.join("tessera/config.json"));
        let _ = std::fs::remove_dir_all(&d);
        // nothing yet: Tessera's
        assert_eq!(config_in(&d), d.join("tessera/config.json"));
    }

    /// The glass is Theme.qml's own: glass_golden.js runs its `glass` object
    /// (preset() and everything it reads) and Glass.qml's sum for one shape
    /// under node, for a grid of settings; every number here is the same
    #[test]
    fn glass_shapes_match_the_shell() {
        let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/glass_golden.js");
        let Ok(out) = std::process::Command::new("node").arg(&script).output() else {
            eprintln!("node not installed: skipped");
            return;
        };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let cases: Value = serde_json::from_slice(&out.stdout).unwrap();
        let cases = cases.as_array().unwrap();
        assert!(cases.len() > 150, "only {} cases", cases.len());
        let mut seen = std::collections::HashSet::new();
        for c in cases {
            let f = |k: &str| c[k].as_f64().unwrap() as f32;
            let mut v = json!({"appearance": c["appearance"].clone()});
            v["appearance"]["look"] = c["look"].clone();
            v["appearance"]["mode"] = json!(if c["dark"].as_bool().unwrap() { "dark" } else { "light" });
            if let Some(s) = c["seed"].as_str() {
                v["appearance"]["materialSeed"] = json!(s);
            }
            let shell = shell_glass_from(&json!({"tone": c["tone"], "cooldown": c["cooldown"]}));
            let look = look_with(&v, None, none, &shell);
            assert_eq!(look.glass.name, c["material"].as_str().unwrap(), "{c}");
            assert_eq!(look.glass.tier(), c["tier"].as_str().unwrap(), "{c}");
            seen.insert(format!("{}/{}", look.glass.name, look.glass.tier()));
            let got = glass_shape(&look, f("x"), f("y"), f("w"), f("h"), f("radius"), f("opacity"), f("lens"));
            let want: Vec<f32> = c["shape"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect();
            if c["tier"] == "solid" {
                // (the QML draws the solid tier itself; here the plugin's solid shape in its colour)
                assert_eq!(got[5], 2.0, "{c}");
                let solid = &look.glass.solid_color;
                for i in 0..3 {
                    assert!((got[12 + i] - solid[i]).abs() < 1e-6, "{c}");
                }
                continue;
            }
            for (i, (g, w)) in got.iter().zip(&want).enumerate() {
                assert!((g - w).abs() <= 2e-4 * w.abs().max(1.0), "value {i}: {g} against the shell's {w} in {c}");
            }
        }
        // (every material and tier came up)
        for m in ["liquid/liquid", "frosted/frosted", "solid/solid", "veil/frosted"] {
            assert!(seen.contains(m), "no {m} case: {seen:?}");
        }
        assert!(seen.iter().any(|m| m.starts_with("custom/")), "{seen:?}");
    }

    /// Fixed numbers from Theme.qml's formulas (glass_golden.js's output, kept
    /// here so the check runs without node): the default Frosted bar, Custom
    /// with water's IOR and the wallpaper's tint, and Liquid's thin depth with
    /// a narrow refraction band at a menu's lens
    #[test]
    fn glass_settings_give_the_shells_numbers() {
        let cases = [
            (json!({}), "#3366cc", (12.0, 30.0, 640.0, 40.0, 0.0, 0.8, 1.0),
             [12.0, 30.0, 640.0, 40.0, 0.0, 0.0, 9.0, 0.165, 0.0, 1.0, 0.084, 0.0924, 0.11441215, 0.11441215, 0.13392844, 0.45000381, 1.275, 0.8, 0.02475]),
            (json!({"glass": "custom", "glassLens": 1, "glassFrost": 0.25, "glassSolid": 0, "glassTint": 1, "glassIor": 1.33}), "#8a5a3c", (12.0, 30.0, 640.0, 40.0, 0.0, 0.8, 1.0),
             [12.0, 30.0, 640.0, 40.0, 0.0, 0.0, 22.330827, 0.4093985, 0.0, 0.25, 0.28, 0.28, 0.12210269, 0.10797284, 0.11766232, 0.33701076, 1.3875, 0.8, 0.7875]),
            (json!({"glass": "liquid", "glassLens": 0.3, "glassFrost": 0, "glassSolid": 1, "glassTint": 0.45, "glassIor": 1.2, "glassRefraction": 0.4, "glassDispersion": 0.35, "glassDepth": "thin", "wallpaperTint": false}), "#808080", (8.701146, 50.0, 141.0, 400.0, 28.0, 1.0, 0.35),
             [8.701146, 50.0, 141.0, 400.0, 28.0, 0.0, 3.85, 0.0, 7.2, 0.75, 0.063, 0.28, 0.10251011, 0.10251011, 0.12251469, 0.39600214, 1.4625, 1.0, 0.3625]),
        ];
        for (appearance, tone, (x, y, w, h, r, o, lens), want) in cases {
            let mut v = json!({"appearance": appearance});
            v["appearance"]["look"] = json!("glass");
            v["appearance"]["mode"] = json!("dark");
            let shell = shell_glass_from(&json!({"tone": tone, "cooldown": "keep"}));
            let got = glass_shape(&look_with(&v, None, none, &shell), x, y, w, h, r, o, lens);
            for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
                assert!((g - w).abs() <= 2e-4 * w.abs().max(1.0), "value {i}: {g} against Theme.qml's {w} for {v}");
            }
        }
        // Reduce transparency and Cool-down's solid glass both make it solid
        for (a, cd) in [(json!({"reduceTransparency": true}), "keep"), (json!({}), "solid")] {
            let mut v = json!({"appearance": a});
            v["appearance"]["look"] = json!("glass");
            let look = look_with(&v, None, none, &shell_glass_from(&json!({"cooldown": cd})));
            assert_eq!(look.glass.tier(), "solid", "{v} {cd}");
            assert_eq!(glass_shape(&look, 0.0, 0.0, 100.0, 40.0, 10.0, 1.0, 1.0)[5], 2.0);
        }
    }

    #[test]
    fn the_shell_says_the_tone_and_cool_down() {
        let g = shell_glass_from(&json!({"src": "shell", "kind": "glass", "tone": "#3366cc", "cooldown": "solid"}));
        assert_eq!(g.cooldown, "solid");
        assert!((g.tone.unwrap()[2] - 0.8).abs() < 1e-6);
        // (anything else: no tone, glass kept)
        assert_eq!(shell_glass_from(&json!({"tone": "blue", "cooldown": "melt"})), ShellGlass::default());
        // Cool-down's solid glass makes Material's panels opaque too
        let v = json!({"appearance": {"look": "material", "materialTransparency": 0.6, "materialSeed": "#3b6ea5"}});
        assert!((look_with(&v, None, none, &ShellGlass::default()).transparency - 0.6).abs() < 1e-6);
        assert_eq!(look_with(&v, None, none, &g).transparency, 0.0);
    }

    /// The Material roles match the shell's own MaterialColor.js (node runs it,
    /// when node is there) for the styles the shell offers, both modes
    #[test]
    fn material_roles_match_the_shell() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shell/common/MaterialColor.js");
        let script = format!(
            r##"const fs=require("fs");const src=fs.readFileSync({:?},"utf8").replace(/^\.pragma.*$/m,"");
               const MCU=new Function(src+";return MCU;")();
               const out={{}};for(const seed of ["#3b6ea5","#d97706","#16a34a"])for(const dark of [true,false])for(const st of ["tonalSpot","vibrant"])
               out[seed+"/"+dark+"/"+st]=MCU.scheme(seed,dark,st,0);console.log(JSON.stringify(out));"##,
            root
        );
        let Ok(out) = std::process::Command::new("node").args(["-e", &script]).output() else {
            eprintln!("node not installed: skipped");
            return;
        };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let js: Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut compared = 0;
        for (key, roles) in js.as_object().unwrap() {
            let mut k = key.split('/');
            let (seed, dark, style) = (k.next().unwrap(), k.next().unwrap() == "true", k.next().unwrap());
            for (name, hex) in material_roles(seed, dark, style) {
                if let Some(want) = roles[&name].as_str() {
                    assert_eq!(hex, want.to_lowercase(), "{key} {name}");
                    compared += 1;
                }
            }
        }
        assert!(compared > 500, "only {compared} roles compared");
    }
}
