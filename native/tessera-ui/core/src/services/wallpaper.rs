//! Is the wallpaper light under the menu bar? (services::wallpaper::WallTop)
//! Decides dark or light text for what sits straight on the wallpaper (the
//! menu bar, the lock screen's clock), as shell/modules/background/
//! Background.qml decides it: the whole picture squeezed into 64 x 64, its
//! top two rows (the top 3%) averaged, light above 0.6 luminance.
//! `sample(path, tx)` reads the picture on a thread and sends the answer.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallTop {
    pub path: String,
    pub light: bool,
}

/// The decision, from the top rows' RGB pixels
pub fn top_is_light(rgb: &[[u8; 3]]) -> bool {
    if rgb.is_empty() {
        return false;
    }
    let sum: f64 = rgb.iter().map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64).sum();
    sum / rgb.len() as f64 / 255.0 > 0.6
}

/// The top rows of a picture squeezed into 64 x 64
pub fn top_rows(path: &std::path::Path) -> Option<Vec<[u8; 3]>> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    // (a box average: a 4K picture in milliseconds, and the top rows are an average anyway)
    let small = img.thumbnail_exact(64, 64).to_rgb8();
    Some((0..2).flat_map(|y| (0..64).map(move |x| (x, y))).map(|(x, y)| small.get_pixel(x, y).0).collect())
}

/// Read the picture's top on a thread; the answer arrives as Msg::Data(WallTop)
pub fn sample(path: &str, tx: Sender<Msg>) {
    let path = path.to_string();
    std::thread::spawn(move || {
        let light = top_rows(std::path::Path::new(&path)).is_some_and(|px| top_is_light(&px));
        let _ = tx.send(Msg::Data(Box::new(WallTop { path, light })));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_above_sixty_percent() {
        assert!(top_is_light(&[[255, 255, 255]; 128]));
        assert!(!top_is_light(&[[20, 30, 40]; 128]));
        // a pale sky: light; a saturated blue (low luminance) isn't
        assert!(top_is_light(&[[200, 220, 240]; 128]));
        assert!(!top_is_light(&[[30, 90, 230]; 128]));
        // half white, half black averages 0.5: dark
        let mut half = vec![[255, 255, 255]; 64];
        half.extend(vec![[0, 0, 0]; 64]);
        assert!(!top_is_light(&half));
        assert!(!top_is_light(&[]));
    }

    #[test]
    fn the_film_pictures() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../site/film/hero/assets");
        let Some(leaves) = top_rows(&root.join("leaves.jpg")) else {
            eprintln!("film pictures missing: skipped");
            return;
        };
        assert_eq!(leaves.len(), 128);
        assert!(!top_is_light(&leaves), "the leaves are dark at the top");
        let d = std::env::temp_dir().join(format!("tessera-wall-{}.png", std::process::id()));
        image::RgbImage::from_pixel(300, 200, image::Rgb([240, 240, 235])).save(&d).unwrap();
        assert!(top_is_light(&top_rows(&d).unwrap()));
        let _ = std::fs::remove_file(&d);
        assert_eq!(top_rows(std::path::Path::new("/nonexistent.jpg")), None);
    }
}
