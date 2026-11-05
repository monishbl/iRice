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
