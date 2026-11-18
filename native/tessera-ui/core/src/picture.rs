//! Pictures for the screen: decoded off the UI thread, cropped and shrunk to
//! the pixels they cover, then handed to Slint. The QML shows the wallpaper
//! and the picker's thumbnails with Image.PreserveAspectCrop (the picture
//! scaled to cover its box, centred, the overflow cut) and a sourceSize of
//! the box, so a picture is never kept larger than it shows; here the same
//! crop is done before the pixels reach Slint, and a picture smaller than its
//! box is kept at its own size (Slint stretches it when drawing: no pixels to
//! gain by upscaling first).

use slint::{Rgb8Pixel, SharedPixelBuffer};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// The part of a `w`×`h` picture that covers a `bw`×`bh` box, scaled to
/// cover it and centred (PreserveAspectCrop): (x, y, width, height) in the
/// picture's pixels
pub fn cover(w: u32, h: u32, bw: u32, bh: u32) -> (u32, u32, u32, u32) {
    if w == 0 || h == 0 || bw == 0 || bh == 0 {
        return (0, 0, w, h);
    }
    let scale = (bw as f64 / w as f64).max(bh as f64 / h as f64);
    let cw = ((bw as f64 / scale).round() as u32).clamp(1, w);
    let ch = ((bh as f64 / scale).round() as u32).clamp(1, h);
    ((w - cw) / 2, (h - ch) / 2, cw, ch)
}

/// The size to keep a cropped `cw`×`ch` part at for a `bw`×`bh` box: the
/// box's, or the part's own when it's smaller (never upscaled)
pub fn kept_size(cw: u32, ch: u32, bw: u32, bh: u32) -> (u32, u32) {
    if cw <= bw || ch <= bh {
        (cw, ch)
    } else {
        (bw, bh)
    }
}

/// A picture ready for Slint, at most the box's pixels
pub struct Decoded {
    pub path: String,
    /// the box it was made for (pixels)
    pub size: (u32, u32),
    /// None: the picture couldn't be read
    pub pixels: Option<SharedPixelBuffer<Rgb8Pixel>>,
}

/// Decode `path` for a `bw`×`bh` box (blocking: on a worker thread). A
/// picture with transparency is laid on black, as the QML's wallpaper is
pub fn decode(path: &str, bw: u32, bh: u32) -> Option<SharedPixelBuffer<Rgb8Pixel>> {
    use image::imageops::{self, FilterType};
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let rgb = if img.color().has_alpha() {
        let mut rgba = img.into_rgba8();
        for p in rgba.pixels_mut() {
            let a = p[3] as u32;
            for c in 0..3 {
                p[c] = ((p[c] as u32 * a + 127) / 255) as u8;
            }
        }
        image::DynamicImage::ImageRgba8(rgba).into_rgb8()
    } else {
        img.into_rgb8()
    };
    let (x, y, cw, ch) = cover(rgb.width(), rgb.height(), bw, bh);
    let (kw, kh) = kept_size(cw, ch, bw, bh);
    let part = imageops::crop_imm(&rgb, x, y, cw, ch);
    let out = if (kw, kh) == (cw, ch) { part.to_image() } else { imageops::resize(&*part, kw, kh, FilterType::Triangle) };
    drop(rgb);
    let buf = SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(out.as_raw(), kw, kh);
    drop(out);
    trim();
    Some(buf)
}

/// Give the decoder's freed memory back to the system now (a 6000×4000
/// picture passes through about 70 MB; glibc would otherwise keep some of it)
fn trim() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
}

// ── A screen's copy, kept for the next start ────────────────────────────────
// The wallpaper as decoded for a screen (cropped and shrunk: `decode`), kept
// in the XDG cache as its raw pixels, so the next start shows it without
// decoding the original again: a read of a few MB instead of a JPEG decode and
// resize (a 2400×1800 one ~0.1 s of the start; a 5K one several times that),
// the background's first frame that much sooner. The key is the picture's
// path, size and modification time and the box: a changed picture is never
// shown from an old copy. The newest KEEP copies stay; a copy that doesn't
// read back whole is ignored (decoded as before).

const COPY_MAGIC: &[u8; 4] = b"TWP1";
const COPY_KEEP: usize = 3;

fn copy_file(dir: &std::path::Path, path: &str, bw: u32, bh: u32) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (path, m.len(), mtime, bw, bh).hash(&mut h);
    Some(dir.join(format!("{:016x}.rgb", h.finish())))
}

/// The copy of `path` kept for a `bw`×`bh` box, if there is a whole one
pub fn read_copy(dir: &std::path::Path, path: &str, bw: u32, bh: u32) -> Option<SharedPixelBuffer<Rgb8Pixel>> {
    use std::io::Read;
    let mut f = std::fs::File::open(copy_file(dir, path, bw, bh)?).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    if &head[..4] != COPY_MAGIC {
        return None;
    }
    let (w, h) = (u32::from_le_bytes(head[4..8].try_into().ok()?), u32::from_le_bytes(head[8..12].try_into().ok()?));
    // (never more than the box: `decode` keeps at most that)
    if w == 0 || h == 0 || w > bw.max(1) || h > bh.max(1) || f.metadata().ok()?.len() != 12 + w as u64 * h as u64 * 3 {
        return None;
    }
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    f.read_exact(buf.make_mut_bytes()).ok()?;
    // (in use: the newest, so pruning keeps it)
    let _ = f.set_modified(std::time::SystemTime::now());
    Some(buf)
}

/// Keep `pixels` (decoded from `path` for a `bw`×`bh` box) for the next
/// start; the oldest copies beyond COPY_KEEP go
pub fn write_copy(dir: &std::path::Path, path: &str, bw: u32, bh: u32, pixels: &SharedPixelBuffer<Rgb8Pixel>) {
    use std::io::Write;
    let Some(file) = copy_file(dir, path, bw, bh) else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = file.with_extension("tmp");
    let ok = (|| -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        f.write_all(COPY_MAGIC)?;
        f.write_all(&pixels.width().to_le_bytes())?;
        f.write_all(&pixels.height().to_le_bytes())?;
        f.write_all(pixels.as_bytes())?;
        f.flush()?;
        // (no sync: a copy cut short reads back as no copy, its length checked)
        std::fs::rename(&tmp, &file)
    })();
    if ok.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut copies: Vec<(std::time::SystemTime, std::path::PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rgb"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    copies.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in copies.into_iter().skip(COPY_KEEP) {
        if p != file {
            let _ = std::fs::remove_file(p);
        }
    }
}

// ── Worker threads ──────────────────────────────────────────────────────────

type Job = Box<dyn FnOnce() + Send>;

struct Queue {
    jobs: VecDeque<Job>,
    closed: bool,
}

/// A few threads that decode, newest wishes first: `replace` drops the
/// waiting jobs (a picture no longer wanted is never decoded); dropping the
/// pool ends the threads once their current job is done
pub struct Pool {
    q: Arc<(Mutex<Queue>, Condvar)>,
}

impl Pool {
    pub fn new(threads: usize, name: &str) -> Self {
        let q = Arc::new((Mutex::new(Queue { jobs: VecDeque::new(), closed: false }), Condvar::new()));
        for i in 0..threads.max(1) {
            let q = q.clone();
            let _ = std::thread::Builder::new().name(format!("{name}-{i}")).spawn(move || loop {
                let job = {
                    let (m, cv) = &*q;
                    let mut g = m.lock().unwrap();
                    loop {
                        if g.closed {
                            return;
                        }
                        if let Some(j) = g.jobs.pop_front() {
                            break j;
                        }
                        g = cv.wait(g).unwrap();
                    }
                };
                job();
            });
        }
        Self { q }
    }

    /// Only these, in this order (the ones waiting are forgotten)
    pub fn replace(&self, jobs: Vec<Job>) {
        let (m, cv) = &*self.q;
        let mut g = m.lock().unwrap();
        g.jobs = jobs.into();
        cv.notify_all();
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let (m, cv) = &*self.q;
        if let Ok(mut g) = m.lock() {
            g.closed = true;
            g.jobs.clear();
        }
        cv.notify_all();
    }
}

/// A window's first frame is drawn before its GL buffer has the window's
/// size (the EGL window starts at 1×1 and is resized as that frame is drawn):
/// the compositor stretches a 1×1 frame over the screen. Something that moves
/// draws again at once; a still picture wouldn't, so it's drawn once more.
/// (A platform matter, egl.rs/wayland.rs; this keeps still surfaces right)
pub fn second_frame(surface: &crate::wayland::Surface, settled: &mut bool) {
    if !*settled && surface.mapped.get() {
        *settled = true;
        surface.window().request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wider_picture_loses_its_sides() {
        // 21:9 on 16:9: full height, the middle 16:9
        assert_eq!(cover(5040, 2160, 3840, 2160), (600, 0, 3840, 2160));
        // 16:9 on a 16:10 screen
        assert_eq!(cover(3840, 2160, 2560, 1600), (192, 0, 3456, 2160));
    }

    #[test]
    fn a_taller_picture_loses_top_and_bottom() {
        // 3:2 on 16:9: full width, the middle band
        assert_eq!(cover(6000, 4000, 1920, 1080), (0, 312, 6000, 3375));
        // portrait on landscape
        assert_eq!(cover(2000, 3000, 1920, 1080), (0, 937, 2000, 1125));
    }

    #[test]
    fn the_same_shape_keeps_everything() {
        assert_eq!(cover(3840, 2160, 1920, 1080), (0, 0, 3840, 2160));
        assert_eq!(cover(1920, 1080, 1920, 1080), (0, 0, 1920, 1080));
        // a smaller picture: all of it, kept at its size (Slint stretches it)
        assert_eq!(cover(1280, 720, 3840, 2160), (0, 0, 1280, 720));
        assert_eq!(kept_size(1280, 720, 3840, 2160), (1280, 720));
    }

    #[test]
    fn kept_at_most_the_box() {
        assert_eq!(kept_size(6000, 3375, 1920, 1080), (1920, 1080));
        assert_eq!(kept_size(1920, 1080, 1920, 1080), (1920, 1080));
        // the thumbnails: a 220×124 box at twice the pixels
        let (x, y, w, h) = cover(6000, 4000, 440, 248);
        assert_eq!((x, y, w, h), (0, 309, 6000, 3382));
        assert_eq!(kept_size(w, h, 440, 248), (440, 248));
    }

    #[test]
    fn odd_sizes_never_panic() {
        assert_eq!(cover(1, 1, 3840, 2160), (0, 0, 1, 1));
        assert_eq!(cover(10000, 1, 1920, 1080), (4999, 0, 2, 1));
        assert_eq!(cover(0, 0, 1920, 1080), (0, 0, 0, 0));
        assert_eq!(cover(1920, 1080, 0, 0), (0, 0, 1920, 1080));
    }

    #[test]
    fn decodes_a_picture_to_its_box() {
        let pic = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../shell/assets/wallpapers/tahoe-dusk.jpg");
        let b = decode(pic, 320, 180).expect("the bundled picture decodes");
        assert_eq!((b.width(), b.height()), (320, 180));
        // (a box the other way round: a portrait screen)
        let b = decode(pic, 180, 320).unwrap();
        assert_eq!((b.width(), b.height()), (180, 320));
        assert!(decode("/nonexistent.jpg", 10, 10).is_none());
    }

    #[test]
    fn decoded_pixels_are_the_picture() {
        // a 16:10 picture on a 16:9 box: the middle band, shrunk
        let pic = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../site/film/hero/assets/screen.jpg");
        let b = decode(pic, 192, 108).unwrap();
        let px = b.as_slice();
        let want = image::open(pic).unwrap().into_rgb8();
        let (x, y, w, h) = cover(want.width(), want.height(), 192, 108);
        let want = image::imageops::resize(&*image::imageops::crop_imm(&want, x, y, w, h), 192, 108, image::imageops::FilterType::Triangle);
        for (i, p) in px.iter().enumerate().step_by(997) {
            let q = want.as_raw();
            assert_eq!((p.r, p.g, p.b), (q[i * 3], q[i * 3 + 1], q[i * 3 + 2]), "pixel {i}");
        }
        let distinct: std::collections::HashSet<(u8, u8, u8)> = px.iter().map(|p| (p.r, p.g, p.b)).collect();
        assert!(distinct.len() > 1000, "{}", distinct.len());
    }

    #[test]
    fn transparent_pixels_go_on_black() {
        let d = std::env::temp_dir().join(format!("tessera-ui-alpha-{}.png", std::process::id()));
        let mut img = image::RgbaImage::new(4, 4);
        for p in img.pixels_mut() {
            *p = image::Rgba([255, 255, 255, 64]);
        }
        img.save(&d).unwrap();
        let b = decode(d.to_str().unwrap(), 4, 4).unwrap();
        let px = b.as_slice()[0];
        assert_eq!((px.r, px.g, px.b), (64, 64, 64));
        let _ = std::fs::remove_file(&d);
    }

    #[test]
    fn a_screen_copy_reads_back_until_the_picture_changes() {
        let root = std::env::temp_dir().join(format!("tessera-ui-copy-{}", std::process::id()));
        let dir = root.join("copies");
        let _ = std::fs::create_dir_all(&root);
        let pic = root.join("wall.png");
        let mut img = image::RgbImage::new(40, 30);
        for (i, p) in img.pixels_mut().enumerate() {
            *p = image::Rgb([(i % 251) as u8, (i % 13) as u8 * 9, 200]);
        }
        img.save(&pic).unwrap();
        let path = pic.to_str().unwrap();
        assert!(read_copy(&dir, path, 16, 9).is_none());
        let px = decode(path, 16, 9).unwrap();
        write_copy(&dir, path, 16, 9, &px);
        let back = read_copy(&dir, path, 16, 9).unwrap();
        assert_eq!((back.width(), back.height(), back.as_bytes()), (px.width(), px.height(), px.as_bytes()));
        // another box: no copy
        assert!(read_copy(&dir, path, 32, 18).is_none());
        // a copy cut short: none
        let file = copy_file(&dir, path, 16, 9).unwrap();
        let bytes = std::fs::read(&file).unwrap();
        std::fs::write(&file, &bytes[..bytes.len() - 1]).unwrap();
        assert!(read_copy(&dir, path, 16, 9).is_none());
        std::fs::write(&file, &bytes).unwrap();
        // the picture changed (its size and time): the old copy isn't it
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut img2 = image::RgbImage::new(41, 30);
        img2.pixels_mut().for_each(|p| *p = image::Rgb([1, 2, 3]));
        img2.save(&pic).unwrap();
        assert!(read_copy(&dir, path, 16, 9).is_none());
        // only the newest few are kept
        for (w, h) in [(8, 8), (9, 9), (10, 10), (11, 11)] {
            // (apart: file times are coarse)
            std::thread::sleep(std::time::Duration::from_millis(20));
            let p = decode(path, w, h).unwrap();
            write_copy(&dir, path, w, h, &p);
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), COPY_KEEP);
        assert!(read_copy(&dir, path, 11, 11).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pool_runs_only_what_is_still_wanted() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let ran = Arc::new(AtomicUsize::new(0));
        let pool = Pool::new(1, "test");
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        // the one thread busy until the gate opens
        let g = gate.clone();
        pool.replace(vec![Box::new(move || {
            let (m, cv) = &*g;
            let mut open = m.lock().unwrap();
            while !*open {
                open = cv.wait(open).unwrap();
            }
        })]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let waiting: Vec<Job> = (0..5)
            .map(|_| {
                let r = ran.clone();
                Box::new(move || {
                    r.fetch_add(1, Ordering::SeqCst);
                }) as Job
            })
            .collect();
        pool.replace(waiting);
        let (tx, rx) = std::sync::mpsc::channel();
        let r = ran.clone();
        pool.replace(vec![Box::new(move || {
            r.fetch_add(100, Ordering::SeqCst);
            let _ = tx.send(());
        })]);
        {
            let (m, cv) = &*gate;
            *m.lock().unwrap() = true;
            cv.notify_all();
        }
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(ran.load(Ordering::SeqCst), 100);
    }
}
