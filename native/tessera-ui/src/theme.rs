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
    let v = read_json();
    // TESSERA_LOOK=glass|material overrides (to measure both looks from one config)
    look_from(&v, crate::env::var("LOOK").as_deref(), wallpaper_seed)
}

/// The look a config says. `wallpaper`: the seed for a picture, when the
/// Material look follows the wallpaper (as the shell, Material's baseline
/// purple when there's none to read)
pub fn look_from(v: &Value, look_override: Option<&str>, wallpaper: impl Fn(&str) -> Option<String>) -> Look {
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
    let transparency = if material && !reduce { a["materialTransparency"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0) as f32 } else { 0.0 };
    Look { material, dark, seed, style: s("materialStyle", "tonalSpot"), accent, contrast, transparency }
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

/// Fill the Slint `Theme` global for a look. `plugin`: the glass plugin
/// draws the glass under the bar (otherwise Hyprland's blur needs more alpha).
pub fn apply(t: &crate::ui::Theme, look: &Look, plugin: bool) {
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
        t.set_m3_secondary(r("secondary"));
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
        t.set_m3_secondary(r("secondary"));
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

/// The plugin's numbers for one glass shape (Theme.glass.preset with the
/// default Liquid material: lens 1, frost 0.75, tint 0.75): x y w h radius
/// mode refraction depth bevel frost chroma rim tintRGBA saturation opacity gloss
/// (`lens` < 1: menus under the bar bend less, MaterialMenu's `lens: 0.35`)
pub fn glass_shape(look: &Look, x: f32, y: f32, w: f32, h: f32, radius: f32, opacity: f32, lens: f32) -> [f32; 19] {
    let thick = w.min(h) <= 140.0;
    let (l, f) = (1.0f32, 0.75f32);
    // Body: the clear tint toward the frosted one (Theme.qml's mixc)
    let (clear, frost): ([f32; 4], [f32; 4]) = if look.dark {
        ([0.10, 0.10, 0.12, 0.216], [0.11, 0.11, 0.13, 0.456])
    } else {
        ([0.96, 0.96, 0.97, 0.243], [0.97, 0.97, 0.98, 0.54])
    };
    let t: Vec<f32> = (0..4).map(|i| clear[i] + (frost[i] - clear[i]) * f).collect();
    [
        x, y, w, h, radius, 0.0,
        if thick { 30.0 } else { 22.0 } * l * lens,
        if thick { 0.55 } else { 0.0 } * l * lens,
        (if thick { 26.0f32 } else { 18.0 }).min(radius),
        f,
        if thick { 0.28 } else { 0.18 } * l * lens,
        0.12 + 0.16 * l,
        t[0], t[1], t[2], t[3],
        1.35 + 0.15 * f,
        opacity,
        l * (1.0 - 0.85 * f),
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

    /// The Material roles match the shell's own MaterialColor.js (node runs it,
    /// when node is there) for the styles the shell offers, both modes
    #[test]
    fn material_roles_match_the_shell() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/common/MaterialColor.js");
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
