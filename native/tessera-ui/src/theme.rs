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
    Look {
        material,
        dark: s("mode", "dark") != "light", // ("auto" follows the sun in the shell; dark here)
        seed,
        style: s("materialStyle", "tonalSpot"),
    }
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
        t.set_pill(r("surfaceContainer"));
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
        let blue = if d { hex("#0A84FF") } else { hex("#007AFF") };
        t.set_menu_hover(blue);
        t.set_menu_hover_fg(Color::from_rgb_u8(255, 255, 255));
        t.set_separator(fg.with_alpha(0.14));
        t.set_field_bg(fg.with_alpha(0.08));
        t.set_field_focus(blue);
        t.set_menu_radius(12.0);
        t.set_item_radius(6.0);
    }
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
