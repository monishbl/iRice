//! Material 3 Expressive's shapes as polar outlines (the QML's MaterialShape
//! draws them with a shader from MaterialShapes.js): one radius per angle, so
//! two shapes morph by mixing their radii, as the QML's do between `from`
//! and `to`. Outlines for Slint's Path (SVG commands in a 0..100 box, or in
//! a window's pixels for the login intro's cover), and pictures cut to a
//! shape on the CPU (your picture in a cookie: Slint can't clip to a path).

use std::f64::consts::{FRAC_PI_2, TAU};

/// The shape's radius at angle `t` (radians from the top, clockwise), 0..1
/// of the box's half
pub fn radius(name: &str, t: f64) -> f64 {
    // (a scalloped wave around a circle, normalised so the tips touch the box)
    let wave = |n: f64, a: f64| (1.0 + a * (n * t).cos()) / (1.0 + a);
    match name {
        "cookie4" => wave(4.0, 0.12),
        "cookie9" => wave(9.0, 0.07),
        "cookie12" => wave(12.0, 0.05),
        "softBurst" => wave(10.0, 0.09),
        "sunny" => wave(8.0, 0.09),
        // (leaves meet in a pinch: a sharper wave than a cosine)
        "clover4" | "clover8" => {
            let n = if name == "clover4" { 4.0 } else { 8.0 };
            let a = if name == "clover4" { 0.2 } else { 0.12 };
            (1.0 + a * ((n * t).cos().abs().powf(0.6) * 2.0 - 1.0)) / (1.0 + a)
        }
        // a rounded square (a superellipse whose sides touch the box)
        "square" => {
            let (c, s) = ((t - FRAC_PI_2).cos().abs(), (t - FRAC_PI_2).sin().abs());
            (c.powi(5) + s.powi(5)).powf(-0.2).min(1.38)
        }
        _ => 1.0,
    }
}

/// Mixed radius: `from` at 0, `to` at 1
pub fn morph_radius(from: &str, to: &str, k: f64, t: f64) -> f64 {
    let (a, b) = (radius(from, t), radius(to, t));
    a + (b - a) * k.clamp(0.0, 1.2)
}

/// The outline in a 0..100 box (`spin`: turned clockwise, degrees)
pub fn outline(from: &str, to: &str, k: f64, spin: f64) -> String {
    path_at(50.0, 50.0, 50.0, from, to, k, spin, true)
}

/// The outline centred on (cx, cy), `half` its box's half size, as SVG
/// commands (`start`: begin with M, else continue the path with another M)
#[allow(clippy::too_many_arguments)]
pub fn path_at(cx: f64, cy: f64, half: f64, from: &str, to: &str, k: f64, spin: f64, close: bool) -> String {
    let steps = 180;
    let mut d = String::new();
    let rot = spin.to_radians();
    for i in 0..=steps {
        let t = i as f64 / steps as f64 * TAU;
        let r = half * morph_radius(from, to, k, t - rot);
        let (x, y) = (cx + r * (t - FRAC_PI_2).cos(), cy + r * (t - FRAC_PI_2).sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    if close {
        d += "Z";
    }
    d
}

/// The Material login intro's cover: the whole `w`×`h` window, with a hole
/// shaped like the opening (even-odd fill: a ring)
pub fn cover_with_hole(w: f64, h: f64, half: f64, from: &str, to: &str, k: f64, spin: f64) -> String {
    let mut d = format!("M 0 0 L {w:.1} 0 L {w:.1} {h:.1} L 0 {h:.1} Z ");
    if half > 0.5 {
        d += &path_at(w / 2.0, h / 2.0, half, from, to, k, spin, true);
    }
    d
}

/// A ring: the shape at `outer` with the same shape cut out at `inner`
/// (even-odd), centred in `w`×`h`
#[allow(clippy::too_many_arguments)]
pub fn ring(w: f64, h: f64, outer: f64, inner: f64, from: &str, to: &str, k: f64, spin: f64) -> String {
    let mut d = path_at(w / 2.0, h / 2.0, outer, from, to, k, spin, true);
    if inner > 0.5 {
        d += " ";
        d += &path_at(w / 2.0, h / 2.0, inner, from, to, k, spin, true);
    }
    d
}

/// A square RGBA picture cut to a shape (antialiased by a pixel), for your
/// picture in a cookie and the album art in Now Playing
pub fn cut(rgba: &[u8], side: u32, shape: &str) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let c = side as f64 / 2.0;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f64 + 0.5 - c, y as f64 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            // angle from the top, clockwise
            let t = (dx.atan2(-dy) + TAU) % TAU;
            let edge = c * radius(shape, t);
            let a = (edge - d + 0.5).clamp(0.0, 1.0);
            let i = ((y * side + x) * 4) as usize;
            if i + 3 < out.len() {
                out[i + 3] = (out[i + 3] as f64 * a).round() as u8;
                // (premultiplied? no: Slint's Rgba8 is straight alpha)
            }
        }
    }
    out
}

/// A picture file as a square RGBA buffer `side` px a side (cropped to
/// cover), or None when it can't be read
pub fn square_picture(path: &str, side: u32) -> Option<Vec<u8>> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let (w, h) = (img.width(), img.height());
    let s = w.min(h);
    if s == 0 {
        return None;
    }
    let sq = img.crop_imm((w - s) / 2, (h - s) / 2, s, s).resize_exact(side, side, image::imageops::FilterType::Triangle);
    Some(sq.to_rgba8().into_raw())
}

/// A buffer for Slint
pub fn to_image(rgba: Vec<u8>, side: u32) -> slint::Image {
    slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, side, side))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fit_their_box() {
        for name in ["circle", "square", "cookie4", "cookie9", "cookie12", "softBurst", "sunny", "clover4", "clover8"] {
            for i in 0..360 {
                let r = radius(name, (i as f64).to_radians());
                assert!(r > 0.6 && r <= 1.4, "{name} {i} {r}");
            }
            let d = outline(name, name, 0.0, 0.0);
            let nums: Vec<f64> = d.split(|c: char| c == ' ' || c.is_ascii_alphabetic()).filter_map(|s| s.parse().ok()).collect();
            assert!(nums.iter().all(|v| (-0.01..=100.01).contains(v)), "{name}");
        }
        // a circle is round; a square's sides touch the box
        assert_eq!(radius("circle", 1.0), 1.0);
        assert!((radius("square", 0.0) - 1.0).abs() < 1e-9);
        assert!(radius("square", std::f64::consts::FRAC_PI_4) > 1.1);
    }

    #[test]
    fn morphs_mix_radii() {
        let t = 0.3;
        assert_eq!(morph_radius("cookie9", "circle", 0.0, t), radius("cookie9", t));
        assert_eq!(morph_radius("cookie9", "circle", 1.0, t), 1.0);
        let mid = morph_radius("square", "circle", 0.5, std::f64::consts::FRAC_PI_4);
        assert!(mid > 1.0 && mid < radius("square", std::f64::consts::FRAC_PI_4));
    }

    #[test]
    fn covers_and_rings() {
        let d = cover_with_hole(200.0, 100.0, 20.0, "circle", "circle", 0.0, 0.0);
        assert!(d.starts_with("M 0 0 L 200.0 0") && d.matches('M').count() == 2);
        // no hole yet: only the rectangle
        assert_eq!(cover_with_hole(200.0, 100.0, 0.0, "circle", "circle", 0.0, 0.0).matches('M').count(), 1);
        assert_eq!(ring(100.0, 100.0, 40.0, 30.0, "cookie9", "circle", 0.5, 10.0).matches('M').count(), 2);
    }

    #[test]
    fn pictures_cut_to_shape() {
        let side = 20;
        let px = vec![255u8; (side * side * 4) as usize];
        let out = cut(&px, side, "circle");
        // corners gone, centre kept
        assert_eq!(out[3], 0);
        let c = ((10 * side + 10) * 4 + 3) as usize;
        assert_eq!(out[c], 255);
    }
}
