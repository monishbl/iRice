//! The wallpaper's settings, read and written as shell/common/Wallpaper.qml
//! does: which picture shows now (the chosen one, the bundled default for the
//! look and mode, the time of day's, the light or dark one of a pair, or a
//! live wallpaper's poster) and the pictures of a time-of-day folder
//! (config.json's `wallpaper` section; Config.qml's defaults when a key is
//! missing). Shared by the background and the picker; choosing a picture is
//! the shell's (`wallpaper.set`, its Wallpaper.set).

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Daylight.qml's defaults until the weather says (6:30 and 18:30)
pub const SUNRISE: i32 = 390;
pub const SUNSET: i32 = 1110;

/// Today's sunrise and sunset as the shell last told them (either part
/// hears them), else the defaults
static SUN: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

pub fn sun() -> (i32, i32) {
    SUN.lock().ok().and_then(|g| *g).unwrap_or((SUNRISE, SUNSET))
}

/// A message from the shell with `sunrise` and `sunset` (minutes): kept;
/// true when they changed
pub fn told_sun(v: &Value) -> bool {
    let (Some(r), Some(s)) = (v["sunrise"].as_i64(), v["sunset"].as_i64()) else { return false };
    let new = Some(((r as i32).clamp(0, 1439), (s as i32).clamp(0, 1439)));
    let Ok(mut g) = SUN.lock() else { return false };
    let changed = *g != new;
    *g = new;
    changed
}

/// Where the day is (Daylight.qml): minutes since midnight, and today's
/// sunrise and sunset (the QML shell's weather service knows them and tells
/// tessera-ui; until then the defaults)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Day {
    pub minute: i32,
    pub sunrise: i32,
    pub sunset: i32,
}

impl Day {
    /// Now, local time
    pub fn now(sunrise: i32, sunset: i32) -> Self {
        let hm = crate::util::now("%H %M");
        let mut it = hm.split(' ').map(|s| s.parse::<i32>().unwrap_or(0));
        let minute = it.next().unwrap_or(0) * 60 + it.next().unwrap_or(0);
        Self { minute, sunrise, sunset }
    }
    pub fn is_day(&self) -> bool {
        self.minute >= self.sunrise && self.minute < self.sunset
    }
}

/// Theme.qml's `dark`: appearance.mode, "auto" following the sun
pub fn dark(cfg: &Value, day: &Day) -> bool {
    match cfg["appearance"]["mode"].as_str().unwrap_or("dark") {
        "auto" => !day.is_day(),
        m => m != "light",
    }
}

fn s<'a>(cfg: &'a Value, key: &str) -> &'a str {
    cfg["wallpaper"][key].as_str().unwrap_or("")
}

/// The wallpaper folder the picker lists (empty: ~/Pictures/Wallpapers)
pub fn directory(cfg: &Value) -> String {
    match s(cfg, "directory") {
        "" => format!("{}/Pictures/Wallpapers", std::env::var("HOME").unwrap_or_default()),
        d => d.to_string(),
    }
}

/// A live wallpaper's video ("" for none)
pub fn live(cfg: &Value) -> &str {
    s(cfg, "live")
}

/// The time-of-day folder, when that mode is on ("" for the bundled set or
/// another mode)
pub fn time_folder(cfg: &Value) -> &str {
    if s(cfg, "dynamic") == "time" {
        s(cfg, "dynamicFolder")
    } else {
        ""
    }
}

/// What the choice of picture depends on besides config.json
pub struct Context<'a> {
    pub material: bool,
    pub day: Day,
    /// a bundled picture by file name (tahoe-dusk.jpg, material-dark.jpg…)
    pub bundled: &'a dyn Fn(&str) -> String,
    /// the time-of-day folder's pictures, in name order
    pub folder: &'a [String],
    /// the live wallpaper's poster frame, once there is one
    pub poster: Option<&'a str>,
}

/// Wallpaper.qml's `path`: the picture showing now
pub fn picture(cfg: &Value, c: &Context) -> String {
    if let Some(p) = c.poster.filter(|p| !p.is_empty() && !live(cfg).is_empty()) {
        return p.to_string();
    }
    let dark = dark(cfg, &c.day);
    let default_dark = (c.bundled)(if c.material { "material-dark.jpg" } else { "tahoe-dusk.jpg" });
    let default_light = (c.bundled)(if c.material { "material-light.jpg" } else { "tahoe-day.jpg" });
    // (Glass: Tahoe dusk whatever the mode; Material: its own pair)
    let still = match s(cfg, "path") {
        "" if c.material => if dark { default_dark.clone() } else { default_light.clone() },
        "" => (c.bundled)("tahoe-dusk.jpg"),
        p => p.to_string(),
    };
    match s(cfg, "dynamic") {
        "time" => time_picture(&c.day, (!s(cfg, "dynamicFolder").is_empty()).then_some(c.folder), c.bundled, &still),
        "appearance" if dark => non_empty(s(cfg, "darkPath")).unwrap_or(default_dark),
        "appearance" => non_empty(s(cfg, "lightPath")).unwrap_or(default_light),
        _ => still,
    }
}

/// The picture showing now, from config.json alone (for the theme's seed, as
/// Wallpaper.qml's matugen reads its `path`): the bundled default when none is
/// chosen, the time-of-day or light/dark one, a live wallpaper's poster
pub fn showing_now(cfg: &Value, material: bool) -> String {
    let tf = time_folder(cfg);
    let folder = if tf.is_empty() { Vec::new() } else { folder_pictures(tf) };
    let lv = live(cfg);
    let poster = (!lv.is_empty()).then(|| poster_path(&cache_dir(), lv)).filter(|p| p.exists()).map(|p| p.to_string_lossy().into_owned());
    let (sunrise, sunset) = sun();
    let bundled = Bundled::find();
    let file = |n: &str| bundled.file(material, n);
    picture(cfg, &Context { material, day: Day::now(sunrise, sunset), bundled: &file, folder: &folder, poster: poster.as_deref() })
}

fn non_empty(p: &str) -> Option<String> {
    (!p.is_empty()).then(|| p.to_string())
}

/// Wallpaper.qml's `timePicture`. The bundled set (no folder) has phases that
/// match its pictures: dawn from an hour before sunrise to an hour and a half
/// after, dusk the same round sunset, day between, night the rest. A folder's
/// pictures (in name order) split in two: the first half spans sunrise to
/// sunset, the rest the night; an empty folder falls back to the still picture
pub fn time_picture(day: &Day, folder: Option<&[String]>, bundled: &dyn Fn(&str) -> String, still: &str) -> String {
    let (m, r, st) = (day.minute, day.sunrise, day.sunset);
    let Some(pics) = folder else {
        // Minutes since an event, wrapped, so phases crossing midnight work
        let since = |t: i32| (m - t).rem_euclid(1440);
        let phase = if since(r - 60) < 150 {
            "dawn"
        } else if since(st - 60) < 150 {
            "dusk"
        } else if since(r + 90) < ((st - 60) - (r + 90)).rem_euclid(1440) {
            "day"
        } else {
            "night"
        };
        return bundled(&format!("tahoe-{phase}.jpg"));
    };
    if pics.is_empty() {
        return still.to_string();
    }
    let n = pics.len() as i32;
    let day_n = (n + 1) / 2;
    let night_n = n - day_n;
    if (m >= r && m < st) || night_n == 0 {
        let at = ((m - r).rem_euclid(1440) as f64 / (st - r).max(1) as f64 * day_n as f64).floor() as i32;
        return pics[at.min(day_n - 1).max(0) as usize].clone();
    }
    let night_len = 1440 - (st - r);
    let at = ((m - st).rem_euclid(1440) as f64 / night_len as f64 * night_n as f64).floor() as i32;
    pics[(day_n + at.min(night_n - 1).max(0)) as usize].clone()
}

/// A time-of-day folder's pictures, in name order (as Wallpaper.qml lists
/// them: `find -L <folder> -maxdepth 1 -type f` of jpg, jpeg, png and webp in
/// any case, sorted)
pub fn folder_pictures(folder: &str) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| is_picture_any_case(&e.file_name().to_string_lossy()))
        .filter(|e| std::fs::metadata(e.path()).is_ok_and(|m| m.is_file()))
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

pub fn is_picture_any_case(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".webp"].iter().any(|x| n.ends_with(x))
}

// ── Where the pictures are ──────────────────────────────────────────────────

/// Tessera's shell folder: $TESSERA_ROOT/shell, else the one this was built
/// from (as the fonts), else the installed one
pub fn shell_dir() -> PathBuf {
    let candidates = [
        crate::env::var("ROOT").map(|r| PathBuf::from(r).join("shell")),
        Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../shell"))),
        Some(PathBuf::from("/usr/share/tessera/shell")),
    ];
    candidates.into_iter().flatten().find(|d| d.join("shell.qml").exists()).unwrap_or_else(|| PathBuf::from("/usr/share/tessera/shell"))
}

/// $XDG_CACHE_HOME/tessera (iRice's folder while the settings still live there,
/// as Paths.qml)
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
    let name = crate::theme::config_path().parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "tessera".into());
    base.join(name)
}

/// Wallpaper.qml's `bundledDir`: the shell's assets/wallpapers. The Material
/// look runs from a root of links (material-root.py) whose folder holds both
/// looks' pictures; its paths are the ones the QML writes and compares, so
/// they're used while that root exists
pub struct Bundled {
    pub glass: PathBuf,
    pub material_root: PathBuf,
    pub material_own: PathBuf,
}

impl Bundled {
    pub fn find() -> Self {
        let shell = shell_dir();
        Self {
            glass: shell.join("assets/wallpapers"),
            material_root: cache_dir().join("material-shell/assets/wallpapers"),
            material_own: shell.join("../material/assets/wallpapers"),
        }
    }

    /// The bundled folder(s) the look lists, in order
    pub fn dirs(&self, material: bool) -> Vec<PathBuf> {
        if !material {
            vec![self.glass.clone()]
        } else if self.material_root.is_dir() {
            vec![self.material_root.clone()]
        } else {
            vec![self.glass.clone(), clean(&self.material_own)]
        }
    }

    /// A bundled picture by name, where the look finds it
    pub fn file(&self, material: bool, name: &str) -> String {
        let dirs = self.dirs(material);
        let p = dirs.iter().map(|d| d.join(name)).find(|p| p.exists()).unwrap_or_else(|| dirs[0].join(name));
        p.to_string_lossy().into_owned()
    }
}

/// `a/b/../c` as `a/c` (no file system access: links stay as they are)
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

// ── A live wallpaper's poster ───────────────────────────────────────────────

/// Where the QML shell keeps a video's poster frame (one frame, a second in,
/// extracted once with ffmpeg): cache/live-poster-<md5 of the path>.jpg, so
/// both find the same file
pub fn poster_path(cache: &Path, video: &str) -> PathBuf {
    cache.join(format!("live-poster-{}.jpg", md5_hex(video.as_bytes())))
}

/// Make the poster if it isn't there (blocking: run it on a thread). The QML's
/// command: `ffmpeg -loglevel error -y -ss 1 -i <video> -frames:v 1 -q:v 3 <out>`
pub fn make_poster(video: &str, out: &Path) -> bool {
    if std::fs::metadata(out).is_ok_and(|m| m.len() > 0) {
        return true;
    }
    if let Some(d) = out.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-ss", "1", "-i", video, "-frames:v", "1", "-q:v", "3"])
        .arg(out)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    ok && std::fs::metadata(out).is_ok_and(|m| m.len() > 0)
}

/// MD5 as lowercase hex (Qt.md5), for the poster's file name
pub fn md5_hex(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    [a0, b0, c0, d0].iter().flat_map(|v| v.to_le_bytes()).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bundled(n: &str) -> String {
        format!("/b/{n}")
    }

    fn ctx<'a>(material: bool, minute: i32, folder: &'a [String], poster: Option<&'a str>) -> Context<'a> {
        Context { material, day: Day { minute, sunrise: SUNRISE, sunset: SUNSET }, bundled: &bundled, folder, poster }
    }

    #[test]
    fn defaults_follow_the_look_and_mode() {
        let noon = 12 * 60;
        assert_eq!(picture(&json!({}), &ctx(false, noon, &[], None)), "/b/tahoe-dusk.jpg");
        // (Glass: dusk even in light mode)
        assert_eq!(picture(&json!({"appearance": {"mode": "light"}}), &ctx(false, noon, &[], None)), "/b/tahoe-dusk.jpg");
        assert_eq!(picture(&json!({}), &ctx(true, noon, &[], None)), "/b/material-dark.jpg");
        assert_eq!(picture(&json!({"appearance": {"mode": "light"}}), &ctx(true, noon, &[], None)), "/b/material-light.jpg");
        // auto: light by day, dark at night
        let auto = json!({"appearance": {"mode": "auto"}});
        assert_eq!(picture(&auto, &ctx(true, noon, &[], None)), "/b/material-light.jpg");
        assert_eq!(picture(&auto, &ctx(true, 23 * 60, &[], None)), "/b/material-dark.jpg");
    }

    #[test]
    fn a_chosen_picture_stays() {
        let v = json!({"wallpaper": {"path": "/p/sea.jpg"}});
        assert_eq!(picture(&v, &ctx(true, 600, &[], None)), "/p/sea.jpg");
        assert_eq!(picture(&v, &ctx(false, 600, &[], None)), "/p/sea.jpg");
    }

    #[test]
    fn a_pair_follows_the_mode() {
        let v = json!({"appearance": {"mode": "auto"}, "wallpaper": {"dynamic": "appearance", "lightPath": "/p/l.jpg", "darkPath": "/p/d.jpg"}});
        assert_eq!(picture(&v, &ctx(false, 12 * 60, &[], None)), "/p/l.jpg");
        assert_eq!(picture(&v, &ctx(false, 2 * 60, &[], None)), "/p/d.jpg");
        // (unset: the bundled pair)
        let v = json!({"appearance": {"mode": "light"}, "wallpaper": {"dynamic": "appearance"}});
        assert_eq!(picture(&v, &ctx(false, 0, &[], None)), "/b/tahoe-day.jpg");
        assert_eq!(picture(&json!({"wallpaper": {"dynamic": "appearance"}}), &ctx(false, 0, &[], None)), "/b/tahoe-dusk.jpg");
    }

    #[test]
    fn the_bundled_set_through_the_day() {
        let v = json!({"wallpaper": {"dynamic": "time", "path": "/p/still.jpg"}});
        let at = |h: i32, m: i32| picture(&v, &ctx(false, h * 60 + m, &[], None));
        // sunrise 6:30, sunset 18:30
        assert_eq!(at(5, 29), "/b/tahoe-night.jpg");
        assert_eq!(at(5, 30), "/b/tahoe-dawn.jpg"); // an hour before sunrise
        assert_eq!(at(7, 59), "/b/tahoe-dawn.jpg");
        assert_eq!(at(8, 0), "/b/tahoe-day.jpg"); // an hour and a half after
        assert_eq!(at(17, 29), "/b/tahoe-day.jpg");
        assert_eq!(at(17, 30), "/b/tahoe-dusk.jpg");
        assert_eq!(at(19, 59), "/b/tahoe-dusk.jpg");
        assert_eq!(at(20, 0), "/b/tahoe-night.jpg");
        assert_eq!(at(0, 0), "/b/tahoe-night.jpg");
    }

    #[test]
    fn phases_crossing_midnight() {
        // (sunrise at 0:30: dawn starts the evening before)
        let day = |minute| Day { minute, sunrise: 30, sunset: 1000 };
        assert_eq!(time_picture(&day(1430), None, &bundled, ""), "/b/tahoe-dawn.jpg");
        assert_eq!(time_picture(&day(100), None, &bundled, ""), "/b/tahoe-dawn.jpg");
        assert_eq!(time_picture(&day(120), None, &bundled, ""), "/b/tahoe-day.jpg");
    }

    #[test]
    fn a_folder_through_the_day() {
        let pics: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| format!("/f/{s}.jpg")).collect();
        let v = json!({"wallpaper": {"dynamic": "time", "dynamicFolder": "/f"}});
        let at = |minute| picture(&v, &ctx(false, minute, &pics, None));
        // five pictures: three by day (6:30-18:30, four hours each), two by night (six hours each)
        assert_eq!(at(390), "/f/a.jpg");
        assert_eq!(at(390 + 239), "/f/a.jpg");
        assert_eq!(at(390 + 240), "/f/b.jpg");
        assert_eq!(at(1109), "/f/c.jpg");
        assert_eq!(at(1110), "/f/d.jpg");
        assert_eq!(at(1110 + 359), "/f/d.jpg");
        assert_eq!(at((1110 + 360) % 1440), "/f/e.jpg");
        assert_eq!(at(389), "/f/e.jpg");
        // one picture: all day and night
        let one = vec!["/f/only.jpg".to_string()];
        assert_eq!(picture(&v, &ctx(false, 0, &one, None)), "/f/only.jpg");
        // an empty folder: the still picture
        let v = json!({"wallpaper": {"dynamic": "time", "dynamicFolder": "/f", "path": "/p/still.jpg"}});
        assert_eq!(picture(&v, &ctx(false, 0, &[], None)), "/p/still.jpg");
    }

    #[test]
    fn a_live_wallpaper_shows_its_poster() {
        let v = json!({"wallpaper": {"live": "/v/waves.mp4", "path": "/p/sea.jpg"}});
        assert_eq!(picture(&v, &ctx(false, 0, &[], Some("/c/poster.jpg"))), "/c/poster.jpg");
        // no poster (yet, or no ffmpeg): the picture
        assert_eq!(picture(&v, &ctx(false, 0, &[], None)), "/p/sea.jpg");
        // a poster left from a video no longer in use
        assert_eq!(picture(&json!({"wallpaper": {"path": "/p/sea.jpg"}}), &ctx(false, 0, &[], Some("/c/poster.jpg"))), "/p/sea.jpg");
    }

    #[test]
    fn md5_as_qt() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
        let long = "/home/someone/Videos/a rather long name for a live wallpaper that spans two blocks.mp4";
        assert_eq!(poster_path(Path::new("/c"), long).extension().unwrap(), "jpg");
        // (ffmpeg-free check of a second block: md5 of 64 'a's)
        assert_eq!(md5_hex(&[b'a'; 64]), "014842d480b571495a4a0363793f7367");
    }

    #[test]
    fn the_sun_as_told() {
        assert!(!told_sun(&json!({"open": "DP-1"})));
        assert!(told_sun(&json!({"sunrise": 400, "sunset": 1100})));
        assert_eq!(sun(), (400, 1100));
        assert!(!told_sun(&json!({"sunrise": 400, "sunset": 1100})));
        assert!(told_sun(&json!({"sunrise": SUNRISE, "sunset": SUNSET})));
    }

    #[test]
    fn config_reads_with_qml_defaults() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(directory(&json!({})), format!("{home}/Pictures/Wallpapers"));
        assert_eq!(directory(&json!({"wallpaper": {"directory": "/w"}})), "/w");
        assert_eq!(live(&json!(null)), "");
        assert_eq!(time_folder(&json!({"wallpaper": {"dynamicFolder": "/f"}})), "");
        assert_eq!(time_folder(&json!({"wallpaper": {"dynamic": "time", "dynamicFolder": "/f"}})), "/f");
        assert!(dark(&json!({}), &Day { minute: 600, sunrise: SUNRISE, sunset: SUNSET }));
    }

    #[test]
    fn folders_list_pictures_in_name_order() {
        let d = std::env::temp_dir().join(format!("tessera-ui-folder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub.jpg")).unwrap();
        for n in ["b.PNG", "a.jpg", "c.webp", "notes.txt", "d.JPEG"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let names: Vec<String> = folder_pictures(d.to_str().unwrap()).iter().map(|p| p.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(names, vec!["a.jpg", "b.PNG", "c.webp", "d.JPEG"]);
        let _ = std::fs::remove_dir_all(&d);
        assert!(folder_pictures("/nonexistent/folder").is_empty());
    }

    #[test]
    fn bundled_pictures_by_look() {
        let b = Bundled { glass: "/s/assets/wallpapers".into(), material_root: "/nonexistent/material-shell/assets/wallpapers".into(), material_own: "/s/../material/assets/wallpapers".into() };
        assert_eq!(b.dirs(false), vec![PathBuf::from("/s/assets/wallpapers")]);
        // (no Material root of links: both folders)
        assert_eq!(b.dirs(true), vec![PathBuf::from("/s/assets/wallpapers"), PathBuf::from("/material/assets/wallpapers")]);
        assert_eq!(b.file(false, "tahoe-day.jpg"), "/s/assets/wallpapers/tahoe-day.jpg");
        // the repo's own: each picture where it lives
        let real = Bundled { material_root: "/nonexistent".into(), ..Bundled::find() };
        assert!(Path::new(&real.file(false, "tahoe-dusk.jpg")).exists());
        assert!(Path::new(&real.file(true, "material-dark.jpg")).exists());
        assert!(Path::new(&real.file(true, "tahoe-night.jpg")).exists());
    }

    #[test]
    fn the_theme_seeds_from_the_picture_showing_with_none_chosen() {
        // (no path chosen: the bundled default the look shows, as the QML
        // seeds from it; the teal-for-red mismatch came from seeding nothing)
        let cfg = json!({"appearance": {"mode": "dark"}, "wallpaper": {}});
        let m = showing_now(&cfg, true);
        assert!(m.ends_with("material-dark.jpg") && std::path::Path::new(&m).exists(), "{m}");
        let g = showing_now(&cfg, false);
        assert!(g.ends_with("tahoe-dusk.jpg") && std::path::Path::new(&g).exists(), "{g}");
        let chosen = json!({"wallpaper": {"path": "/pictures/sea.jpg"}});
        assert_eq!(showing_now(&chosen, true), "/pictures/sea.jpg");
    }
}
