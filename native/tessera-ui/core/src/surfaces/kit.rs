//! Small helpers the smaller surfaces share (overlay, welcome, resources,
//! cheatsheet, osk, hotcorners, tiling, decorations): an output's size, an
//! icon in both looks, colours from the shell's hex strings, the short
//! clock, memory in GB.

use crate::wayland::State;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;

/// An output's size in logical px (the screen a surface goes on)
pub fn screen_size(st: &State, output: &WlOutput) -> (f32, f32) {
    let Some(info) = st.outputs.info(output) else { return (1920.0, 1080.0) };
    if let Some((w, h)) = info.logical_size {
        return (w.max(1) as f32, h.max(1) as f32);
    }
    let (w, h) = info.modes.iter().find(|m| m.current).map_or((1920, 1080), |m| m.dimensions);
    let s = info.scale_factor.max(1);
    ((w / s).max(1) as f32, (h / s).max(1) as f32)
}

/// A Phosphor icon by the shell's name, and the Material Symbol the Material
/// look draws in its place (Icon.qml): (phosphor, symbol). A surface turns
/// it into its own Slint `Gl` (@tessera/small-icons.slint) with `gl_fn!`
pub fn glyphs(name: &str) -> (&'static str, &'static str) {
    let p = crate::icons::glyph(name);
    (p, crate::icons::symbol(p))
}

/// `fn gl(name) -> ui::Gl` for a surface crate whose Slint exports `Gl`
#[macro_export]
macro_rules! gl_fn {
    ($ui:ident) => {
        /// An icon in both looks (@tessera/small-icons.slint's Gl)
        pub fn gl(name: &str) -> $ui::Gl {
            let (p, m) = tessera_core::surfaces::kit::glyphs(name);
            $ui::Gl { p: p.into(), m: m.into() }
        }
    };
}

/// "#RRGGBB" / "#AARRGGBB": the shell's hex colours
pub fn hex(s: &str) -> slint::Color {
    let h = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    match h.len() {
        6 => slint::Color::from_argb_u8(255, (v >> 16) as u8, (v >> 8) as u8, v as u8),
        8 => slint::Color::from_argb_u8((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8),
        _ => slint::Color::from_argb_u8(255, 0, 0, 0),
    }
}

/// The wall clock as the shell writes hours and minutes (TimeFormat.hmShort:
/// "HH:mm" with the 24-hour setting, else "h:mm": Qt's h without AP is the
/// hour 0–23 unpadded ("9:05", "21:05")
pub fn hm_short(cfg: &serde_json::Value) -> String {
    let h24 = cfg["appearance"]["clock24"].as_bool().unwrap_or(false);
    let t = crate::util::now("%H:%M");
    if h24 {
        return t;
    }
    let (h, m) = t.split_once(':').unwrap_or(("0", "00"));
    let h: u32 = h.parse().unwrap_or(0);
    format!("{h}:{m}")
}

/// /proc/meminfo: "6.1 of 15.4 GB" (Resources' memory detail)
pub fn mem_detail(info: &str) -> String {
    let kb = |k: &str| -> f64 { info.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(':'))).and_then(|r| r.split_whitespace().next()?.parse().ok()).unwrap_or(0.0) };
    let (total, avail) = (kb("MemTotal"), kb("MemAvailable"));
    format!("{:.1} of {:.1} GB", (total - avail) / 1_048_576.0, total / 1_048_576.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_from_hex() {
        assert_eq!(hex("#30D158"), slint::Color::from_argb_u8(255, 0x30, 0xd1, 0x58));
        assert_eq!(hex("80FFFFFF"), slint::Color::from_argb_u8(0x80, 255, 255, 255));
        assert_eq!(hex("bad"), slint::Color::from_argb_u8(255, 0, 0, 0));
    }

    #[test]
    fn memory_and_icons() {
        assert_eq!(mem_detail("MemTotal: 16148844 kB\nMemAvailable: 9759008 kB\n"), "6.1 of 15.4 GB");
        assert_eq!(glyphs("x").0, crate::icons::glyph("x"));
    }
}
