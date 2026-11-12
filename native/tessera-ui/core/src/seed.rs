//! The Material look's seed from the wallpaper: the colour matugen picks
//! (`matugen image <picture> --source-color-index 0`, as shell/common/
//! Wallpaper.qml runs it), computed here instead of in a second process. The
//! steps are matugen 4.2's (src/color/color.rs, get_scored_colors_from_image),
//! on the same material-colors release it uses (0.4, with its image reader):
//! the picture shrunk to fit 112 x 112 (Triangle filter), its opaque pixels
//! quantized to 128 colours (Celebi), the greys dropped (chroma under 5), then
//! ranked as Material You does (Score); the first is the seed.

use mc04::color::Argb;
use mc04::hct::Cam16;
use mc04::image::{AsPixels, FilterType, ImageReader};
use mc04::quantize::{Quantizer, QuantizerCelebi};
use mc04::score::Score;

/// `#rrggbb`, or None when the picture can't be read or has no colour at all
pub fn from_picture(path: &std::path::Path) -> Option<String> {
    let mut original = ImageReader::open(path.to_str()?).ok()?;
    let image = original.resize(112, 112, FilterType::Triangle);
    let pixels: Vec<Argb> = image.as_pixels().iter().copied().filter(|argb| argb.alpha == 255).collect();
    if pixels.is_empty() {
        return None;
    }
    let mut result = QuantizerCelebi::quantize(&pixels, 128);
    result.color_to_count.retain(|&argb, _| Cam16::from(argb).chroma >= 5.0);
    let ranked = Score::score(&result.color_to_count, None, None, None);
    let c = ranked.first()?;
    Some(format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue))
}

/// The wallpaper's tone as the QML shell finds it (Wallpaper.qml: Quickshell's
/// ColorQuantizer, depth 3, rescaleSize 64, then pickAccent): the picture
/// shrunk to fit 64 x 64, its opaque pixels cut by the median 3 times (8
/// boxes, each split on its widest channel), each box's average; the most
/// colourful one wins (HSV saturation × (0.4 + value)). `#rrggbb`.
/// (The Glass look's wallpaper accent snaps this, not the Material seed.)
pub fn tone_from_picture(path: &std::path::Path) -> Option<String> {
    let img = image::open(path).ok()?;
    let img = if img.width() > 64 || img.height() > 64 { img.resize(64, 64, image::imageops::FilterType::Triangle) } else { img };
    let px: Vec<[u8; 3]> = img.to_rgba8().pixels().filter(|p| p[3] != 0).map(|p| [p[0], p[1], p[2]]).collect();
    if px.is_empty() {
        return None;
    }
    let mut boxes = Vec::new();
    quantize(px, 0, 3, &mut boxes);
    let mut best = [128u8, 128, 128];
    let mut score = -1.0f64;
    for c in boxes {
        let (_, s, v) = hsv(c);
        let sc = s * (0.4 + v);
        if sc > score {
            score = sc;
            best = c;
        }
    }
    Some(format!("#{:02x}{:02x}{:02x}", best[0], best[1], best[2]))
}

fn quantize(mut v: Vec<[u8; 3]>, depth: u32, max: u32, out: &mut Vec<[u8; 3]>) {
    if v.is_empty() {
        return;
    }
    if depth >= max {
        let n = v.len() as f64;
        let avg = |i: usize| (v.iter().map(|c| c[i] as f64).sum::<f64>() / n).round() as u8;
        out.push([avg(0), avg(1), avg(2)]);
        return;
    }
    // (the widest channel; a tie goes to red, then green, as Quickshell's)
    let range = |i: usize| v.iter().map(|c| c[i]).max().unwrap() - v.iter().map(|c| c[i]).min().unwrap();
    let (r, g, b) = (range(0), range(1), range(2));
    let big = r.max(g).max(b);
    let ch = if big == r { 0 } else if big == g { 1 } else { 2 };
    v.sort_by_key(|c| c[ch]);
    let right = v.split_off(v.len() / 2);
    quantize(v, depth + 1, max, out);
    quantize(right, depth + 1, max, out);
}

/// (hue 0..1, saturation 0..1, value 0..1), as QColor's hsvHue/Saturation/Value
pub fn hsv(c: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max > 0.0 { d / max } else { 0.0 };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, max)
}

#[cfg(test)]
mod tone_tests {
    use super::*;

    #[test]
    fn hsv_as_qt() {
        let (h, s, v) = hsv([255, 159, 10]); // #FF9F0A
        assert!((h - 0.1014).abs() < 0.002 && (s - 0.96).abs() < 0.01 && v == 1.0);
        assert_eq!(hsv([128, 128, 128]).1, 0.0);
    }

    #[test]
    fn the_most_colourful_box_wins() {
        let mut boxes = Vec::new();
        let mut px = vec![[20u8, 20, 20]; 100];
        px.extend(vec![[250u8, 150, 20]; 30]);
        quantize(px, 0, 3, &mut boxes);
        assert!(boxes.contains(&[250, 150, 20]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    /// The same seed matugen gives, for the pictures Tessera's own film and
    /// site use (when matugen is installed; it's the shell's own dependency)
    #[test]
    fn matches_matugen() {
        let Ok(out) = std::process::Command::new("matugen").arg("--version").output() else {
            eprintln!("matugen not installed: skipped");
            return;
        };
        assert!(out.status.success());
        let pictures = [
            "site/film/hero/assets/leaves.jpg",
            "site/film/hero/assets/p-ocean.jpg",
            "site/film/hero/assets/p-petals.jpg",
            "site/film/tessera/capture/seed/photos/wall-tiles.jpg",
            "site/film/tessera/capture/seed/photos/lily-photo.jpg",
        ];
        for p in pictures {
            let path = repo().join(p);
            let out = std::process::Command::new("matugen")
                .args(["image", path.to_str().unwrap(), "--json", "hex", "--dry-run", "--source-color-index", "0"])
                .output()
                .expect("matugen");
            let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("matugen's json");
            let c = &v["colors"]["source_color"];
            let want = c["default"]["color"].as_str().or(c["dark"]["color"].as_str()).expect("a source colour").to_lowercase();
            assert_eq!(from_picture(&path).as_deref(), Some(want.as_str()), "{p}");
        }
    }

    #[test]
    fn unreadable_picture_gives_none() {
        assert_eq!(from_picture(std::path::Path::new("/nonexistent/picture.jpg")), None);
    }
}
