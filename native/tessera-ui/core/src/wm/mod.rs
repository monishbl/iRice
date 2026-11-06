//! What the window-management surfaces share (overview, missioncontrol,
//! switcher, stagemanager, launchpad, tabs): app icons by window class, the
//! windows' pictures (services::capture), the wallpaper as a small picture,
//! glyphs as each look draws them, the spaces (spaces.rs) and the shell's
//! own actions they ask for.
//!
//! What only the QML shell does (Spaces' moves, Apps.show for a minimized
//! window, pinning to the dock, Stage Manager's stages, opening the App
//! Store) is asked with one call, `wm`, that RustUi.qml hands to
//! modules/overview/RustWindows.qml (`RustUi.wmCall`), which also tells the
//! surfaces what they show from the shell: the spaces' blocks and names,
//! whether the screen is locked, Stage Manager's waiting stages.

pub mod spaces;

use crate::appicon::{self, Style};
use crate::services::apps::Apps;
use crate::services::capture::{Capturer, Shot};
use crate::services::icons::Icons;
use crate::wayland::State;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub type Px = SharedPixelBuffer<Rgba8Pixel>;

/// Ask the shell for one of its window-management actions (RustWindows.qml)
pub fn ask(what: &str, args: serde_json::Value) {
    let mut a = vec![serde_json::Value::from(what)];
    if let Some(list) = args.as_array() {
        a.extend(list.iter().cloned());
    }
    crate::shell::ask("wm", serde_json::Value::Array(a));
}

/// A glyph as Icon.qml draws it: Material's Material Symbol when there's one
/// (1.2× the size), else Phosphor's: (text, font family, size factor), for a
/// surface's `WGlyph` (@tessera/wm.slint)
pub fn glyph(name: &str, material: bool, bold: bool) -> (slint::SharedString, slint::SharedString, f32) {
    let g = crate::icons::glyph(name);
    let sym = crate::icons::symbol(g);
    if material && !sym.is_empty() {
        return (sym.into(), "Tessera Symbols".into(), 1.2);
    }
    (g.into(), if bold { "Phosphor-Bold" } else { "Phosphor" }.into(), 1.0)
}

/// Glass's panels: over the plugin's glass nothing (the plugin tints it);
/// without it, the frosted fallback's tint (Theme.glass.frost) over
/// Hyprland's blur (as Search's panels)
pub fn glass_fill(plugin: bool, dark: bool) -> slint::Color {
    let c = |r: f32, g: f32, b: f32, a: f32| slint::Color::from_argb_f32(a, r, g, b);
    match (plugin, dark) {
        (true, _) => c(0.0, 0.0, 0.0, 0.0),
        (false, true) => c(0.11, 0.11, 0.13, 0.62),
        (false, false) => c(0.97, 0.97, 0.98, 0.66),
    }
}

/// A rectangle in a surface (logical px)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn has(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

pub fn rgb(c: slint::Color) -> [u8; 3] {
    [c.red(), c.green(), c.blue()]
}

pub fn hexc(s: &str) -> slint::Color {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// "0x…" for any address
pub fn addr(a: &str) -> String {
    if a.starts_with("0x") {
        a.to_string()
    } else {
        format!("0x{a}")
    }
}

// ── App icons ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub path: PathBuf,
    pub px: u32,
    pub style: Style,
}

/// Icons drawn off the UI thread (a part's message)
pub struct IconsDone(pub Vec<(IconKey, Option<Px>)>);

/// App icons by window class or desktop id (Apps.iconFor), each drawn at
/// its size as AppIcon.qml (styled) or IconImage (plain) draws it
#[derive(Default)]
pub struct AppIcons {
    cache: HashMap<IconKey, slint::Image>,
    pending: HashSet<IconKey>,
    queued: Vec<IconKey>,
    styled: Option<Style>,
    plain: Option<Style>,
}

impl AppIcons {
    /// The look changed: draw them again (the surface's Theme: its accent,
    /// primaryContainer and onPrimaryContainer)
    pub fn restyle(&mut self, st: &State, accent: slint::Color, container: slint::Color, on_container: slint::Color) {
        let themed = (rgb(container), rgb(on_container));
        let (m, dark) = (st.shell.look.material, st.shell.look.dark);
        self.styled = Some(Style::from_settings(&st.shell.cfg, m, dark, rgb(accent), themed));
        self.plain = Some(Style::from_settings(&serde_json::json!({"appearance": {"iconStyle": "plain", "iconShape": "none"}}), m, dark, [0, 0, 0], ([0, 0, 0], [0, 0, 0])));
        self.cache.clear();
        self.pending.clear();
    }

    /// Apps.iconFor: the entry's icon (a name or a file), else the class itself
    pub fn path(st: &State, class_or_id: &str) -> Option<PathBuf> {
        let apps = st.shell.get::<Apps>();
        let icons = st.shell.get::<Icons>()?;
        let entry = apps.and_then(|a| a.by_id(class_or_id).or_else(|| a.entry_for(class_or_id)));
        let name = entry.map(|e| e.icon.clone()).filter(|i| !i.is_empty()).unwrap_or_else(|| class_or_id.to_string());
        if class_or_id == "org.quickshell" {
            return icons.path(&["preferences-system"], &["application-x-executable"]);
        }
        if name.starts_with('/') {
            return Some(PathBuf::from(name));
        }
        icons.path(&[name.as_str()], &["application-x-executable"])
    }

    /// The picture for this class at `size` logical px, if drawn yet (else
    /// queued: `flush` starts it)
    pub fn get(&mut self, st: &State, class: &str, size: f32, scale: f32, styled: bool) -> Option<slint::Image> {
        let style = if styled { self.styled.clone()? } else { self.plain.clone()? };
        let path = Self::path(st, class)?;
        let key = IconKey { path, px: (size * scale.max(1.0)).ceil().max(8.0) as u32, style };
        if let Some(i) = self.cache.get(&key) {
            return Some(i.clone());
        }
        if !self.pending.contains(&key) && !self.queued.contains(&key) {
            self.queued.push(key);
        }
        None
    }

    /// Draw what's queued (the part hears IconsDone)
    pub fn flush(&mut self, to: impl Fn(Box<dyn Any + Send>) + Send + 'static) {
        if self.queued.is_empty() {
            return;
        }
        let list: Vec<IconKey> = std::mem::take(&mut self.queued);
        self.pending.extend(list.iter().cloned());
        std::thread::spawn(move || {
            let out = list.into_iter().map(|k| {
                let img = appicon::render(&k.path, k.px, &k.style, None);
                (k, img)
            });
            to(Box::new(IconsDone(out.collect())));
        });
    }

    pub fn done(&mut self, d: IconsDone) {
        for (k, px) in d.0 {
            self.pending.remove(&k);
            if let Some(px) = px {
                self.cache.insert(k, slint::Image::from_rgba8(px));
            }
        }
        if self.cache.len() > 600 {
            self.cache.clear();
        }
    }
}

// ── Window pictures ─────────────────────────────────────────────────────────

/// The windows' pictures while a surface shows them (WindowPreview.qml's
/// ScreencopyView): taken when asked, kept until `clear` (after the close
/// animation), live ones taken again about 30 times a second (LiveCapture)
#[derive(Default)]
pub struct Thumbs {
    cap: Option<Capturer>,
    pub images: HashMap<String, slint::Image>,
    pub failed: HashSet<String>,
    asked: HashSet<String>,
    live: HashMap<String, (u32, u32)>,
    gen: u64,
    timer: Option<slint::Timer>,
    /// how often live pictures are taken again (ms)
    pub interval: u64,
}

/// Time for the live pictures (a part's message)
pub struct LiveTick;

impl Thumbs {
    fn capturer(&mut self, st: &State, part: &'static str) -> &Capturer {
        if self.cap.is_none() {
            let to = st.shell.sender(part);
            self.cap = Some(Capturer::spawn(move |s: Shot| to(Box::new(s))));
        }
        self.cap.as_ref().unwrap()
    }

    /// A picture of this window, at most `max` device pixels (once per
    /// clear, unless it's live)
    pub fn ask(&mut self, st: &State, part: &'static str, address: &str, max: (u32, u32)) {
        if self.images.contains_key(address) || self.failed.contains(address) || self.asked.contains(address) || max.0 == 0 || max.1 == 0 {
            return;
        }
        self.asked.insert(address.to_string());
        let gen = self.gen;
        self.capturer(st, part).ask(address, max, gen);
    }

    /// Take these again and again (the rest: once), at `interval` ms
    pub fn set_live(&mut self, st: &State, part: &'static str, live: HashMap<String, (u32, u32)>) {
        self.live = live;
        if self.live.is_empty() {
            self.timer = None;
            return;
        }
        if self.timer.is_none() {
            let t = slint::Timer::default();
            let to = st.shell.sender(part);
            let ms = if self.interval == 0 { 33 } else { self.interval };
            t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(ms), move || to(Box::new(LiveTick)));
            self.timer = Some(t);
        }
    }

    pub fn tick(&mut self, st: &State, part: &'static str) {
        let want: Vec<(String, (u32, u32))> = self.live.iter().filter(|(a, _)| !self.asked.contains(*a)).map(|(a, m)| (a.clone(), *m)).collect();
        for (a, m) in want {
            self.asked.insert(a.clone());
            let gen = self.gen;
            self.capturer(st, part).ask(&a, m, gen);
        }
    }

    /// A picture came; true when it's one to show
    pub fn got(&mut self, s: Shot) -> bool {
        if s.tag != self.gen {
            return false;
        }
        self.asked.remove(&s.address);
        match s.px {
            Some(px) => {
                self.failed.remove(&s.address);
                self.images.insert(s.address, slint::Image::from_rgba8(px));
            }
            None => {
                if !self.images.contains_key(&s.address) {
                    self.failed.insert(s.address);
                }
            }
        }
        true
    }

    /// Forget every picture (the surface closed), and stop the live ones
    pub fn clear(&mut self) {
        self.gen += 1;
        self.images.clear();
        self.failed.clear();
        self.asked.clear();
        self.live.clear();
        self.timer = None;
    }

    /// Take them again (reopened before the last close finished)
    pub fn retake(&mut self) {
        self.gen += 1;
        self.failed.clear();
        self.asked.clear();
        self.images.clear();
    }
}

// ── The wallpaper ───────────────────────────────────────────────────────────

/// The wallpaper as a small picture (Image's sourceSize), for the spaces'
/// miniatures (overview.wallpapers)
pub struct WallPicture(pub String, pub Option<Px>);

#[derive(Default)]
pub struct Wall {
    pub path: String,
    pub image: Option<slint::Image>,
    asked: String,
}

impl Wall {
    pub fn want(&mut self, st: &State, part: &'static str, size: (u32, u32)) {
        let p = crate::wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        if p == self.asked {
            return;
        }
        self.asked = p.clone();
        if p.is_empty() {
            self.path.clear();
            self.image = None;
            return;
        }
        let to = st.shell.sender(part);
        std::thread::spawn(move || {
            let px = image::open(&p).ok().map(|i| {
                let t = i.resize_to_fill(size.0, size.1, image::imageops::FilterType::Triangle).to_rgba8();
                let mut b = Px::new(t.width(), t.height());
                b.make_mut_bytes().copy_from_slice(t.as_raw());
                b
            });
            to(Box::new(WallPicture(p, px)));
        });
    }

    pub fn got(&mut self, w: WallPicture) {
        if w.0 == self.asked {
            self.path = w.0;
            self.image = w.1.map(slint::Image::from_rgba8);
        }
    }
}

/// overview.* settings, with Config.qml's defaults
#[derive(Clone, Debug, PartialEq)]
pub struct OverviewCfg {
    /// snapshot | live | hybrid
    pub previews: String,
    /// grid | macos
    pub style: String,
    pub wallpapers: bool,
    /// rows | each
    pub displays: String,
}

impl OverviewCfg {
    pub fn read(cfg: &serde_json::Value) -> Self {
        let o = &cfg["overview"];
        let s = |k: &str, d: &str| o[k].as_str().unwrap_or(d).to_string();
        let previews = s("previews", "snapshot");
        Self {
            previews: if previews == "live" || previews == "hybrid" { previews } else { "snapshot".into() },
            style: s("style", "grid"),
            wallpapers: o["wallpapers"].as_bool().unwrap_or(false),
            displays: s("displays", "rows"),
        }
    }
}

/// menubar.workspaceCount (Config.qml: 5)
pub fn workspace_count(cfg: &serde_json::Value) -> i32 {
    cfg["menubar"]["workspaceCount"].as_i64().unwrap_or(5) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_with_the_shells_defaults() {
        let c = OverviewCfg::read(&json!({}));
        assert_eq!((c.previews.as_str(), c.style.as_str(), c.wallpapers, c.displays.as_str()), ("snapshot", "grid", false, "rows"));
        let c = OverviewCfg::read(&json!({"overview": {"previews": "nonsense", "style": "macos", "wallpapers": true}}));
        assert_eq!((c.previews.as_str(), c.style.as_str(), c.wallpapers), ("snapshot", "macos", true));
        assert_eq!(workspace_count(&json!({"menubar": {"workspaceCount": 8}})), 8);
        assert_eq!(addr("5f1"), "0x5f1");
        assert_eq!(addr("0x5f1"), "0x5f1");
    }

}
